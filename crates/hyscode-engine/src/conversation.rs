//! Gerenciamento de conversas e persistência em SQLite.
//!
//! Armazena conversas e mensagens localmente para histórico persistente.

use hyscode_core::models::message::Message;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use std::path::Path;
use tracing::{debug, error, info};

/// Gerencia o estado e a persistência de conversas.
pub struct ConversationManager {
    pool: Pool<Sqlite>,
}

impl ConversationManager {
    /// Inicializa o ConversationManager criando/abrindo o banco SQLite.
    pub async fn new(db_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_path = db_path.as_ref();

        // Garante que o diretório pai existe e cria o arquivo vazio se necessário.
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if !db_path.exists() {
            tokio::fs::File::create(db_path).await?;
        }

        let database_url = format!(
            "sqlite:///{}",
            db_path.to_string_lossy().replace('\\', "/")
        );
        info!("Inicializando ConversationManager em {}", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        Self::run_migrations(&pool).await?;

        Ok(Self { pool })
    }

    /// Cria as tabelas necessárias se não existirem.
    async fn run_migrations(pool: &Pool<Sqlite>) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch())
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_call_id TEXT,
                is_error INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Índice para busca rápida de mensagens por conversa.
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_messages_conversation_id
            ON messages(conversation_id)
            "#,
        )
        .execute(pool)
        .await?;

        debug!("Migrações do ConversationManager aplicadas com sucesso");
        Ok(())
    }

    /// Cria uma nova conversa e retorna seu ID.
    pub async fn create(&self, provider: &str, model: &str) -> anyhow::Result<String> {
        let id = ulid::Ulid::new().to_string();

        sqlx::query(
            r#"
            INSERT INTO conversations (id, provider, model)
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(&id)
        .bind(provider)
        .bind(model)
        .execute(&self.pool)
        .await?;

        info!(
            conversation_id = %id,
            provider = %provider,
            model = %model,
            "Nova conversa criada"
        );

        Ok(id)
    }

    /// Adiciona uma mensagem à conversa.
    pub async fn add_message(
        &self,
        conversation_id: &str,
        message: &Message,
    ) -> anyhow::Result<()> {
        let (role, content, tool_calls, tool_call_id, is_error) = match message {
            Message::System { content } => ("system", content.clone(), None, None, false),
            Message::User { content } => {
                let text = match content {
                    hyscode_core::models::message::MessageContent::Text(t) => t.clone(),
                    hyscode_core::models::message::MessageContent::Parts(parts) => {
                        serde_json::to_string(parts).unwrap_or_default()
                    }
                };
                ("user", text, None, None, false)
            }
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let tc_json = tool_calls
                    .as_ref()
                    .map(|tc| serde_json::to_string(tc).unwrap_or_default());
                let content_text = content.clone().unwrap_or_default();
                ("assistant", content_text, tc_json, None, false)
            }
            Message::Tool {
                tool_call_id,
                content,
                is_error,
            } => (
                "tool",
                content.clone(),
                None,
                Some(tool_call_id.clone()),
                *is_error,
            ),
        };

        sqlx::query(
            r#"
            INSERT INTO messages (conversation_id, role, content, tool_calls, tool_call_id, is_error)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(tool_calls)
        .bind(tool_call_id)
        .bind(if is_error { 1 } else { 0 })
        .execute(&self.pool)
        .await?;

        // Atualiza o timestamp da conversa.
        sqlx::query(
            r#"
            UPDATE conversations
            SET updated_at = unixepoch()
            WHERE id = ?1
            "#,
        )
        .bind(conversation_id)
        .execute(&self.pool)
        .await?;

        debug!(conversation_id = %conversation_id, role = %role, "Mensagem adicionada");
        Ok(())
    }

    /// Carrega o histórico de mensagens de uma conversa.
    pub async fn load_messages(&self, conversation_id: &str) -> anyhow::Result<Vec<Message>> {
        let rows = sqlx::query(
            r#"
            SELECT role, content, tool_calls, tool_call_id, is_error
            FROM messages
            WHERE conversation_id = ?1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;

        let mut messages = Vec::with_capacity(rows.len());

        for row in rows {
            let role: String = row.try_get("role")?;
            let content: String = row.try_get("content")?;
            let tool_calls_json: Option<String> = row.try_get("tool_calls")?;
            let tool_call_id: Option<String> = row.try_get("tool_call_id")?;
            let is_error: i32 = row.try_get("is_error")?;

            let message = match role.as_str() {
                "system" => Message::System { content },
                "user" => {
                    // Tenta desserializar como ContentPart array, senão usa Text
                    let content = if content.starts_with('[') {
                        match serde_json::from_str::<Vec<hyscode_core::models::message::ContentPart>>(
                            &content,
                        ) {
                            Ok(parts) => {
                                hyscode_core::models::message::MessageContent::Parts(parts)
                            }
                            Err(_) => hyscode_core::models::message::MessageContent::Text(content),
                        }
                    } else {
                        hyscode_core::models::message::MessageContent::Text(content)
                    };
                    Message::User { content }
                }
                "assistant" => {
                    let tool_calls = tool_calls_json.as_deref().and_then(|json| {
                        serde_json::from_str::<Vec<hyscode_core::models::tool::ToolCall>>(json).ok()
                    });
                    Message::Assistant {
                        content: Some(content).filter(|s: &String| !s.is_empty()),
                        tool_calls,
                        thinking: None,
                    }
                }
                "tool" => Message::Tool {
                    tool_call_id: tool_call_id.unwrap_or_default(),
                    content,
                    is_error: is_error != 0,
                },
                _ => {
                    error!("Role desconhecido no banco: {}", role);
                    continue;
                }
            };

            messages.push(message);
        }

        Ok(messages)
    }

    /// Lista conversas recentes.
    pub async fn list_recent(&self, limit: u32) -> anyhow::Result<Vec<ConversationSummary>> {
        let rows = sqlx::query(
            r#"
            SELECT
                c.id,
                c.title,
                c.provider,
                c.model,
                c.created_at,
                COUNT(m.id) as message_count
            FROM conversations c
            LEFT JOIN messages m ON c.id = m.conversation_id
            GROUP BY c.id
            ORDER BY c.updated_at DESC
            LIMIT ?1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut summaries = Vec::with_capacity(rows.len());
        for row in rows {
            summaries.push(ConversationSummary {
                id: row.try_get("id")?,
                title: row.try_get("title")?,
                provider: row.try_get("provider")?,
                model: row.try_get("model")?,
                created_at: row.try_get("created_at")?,
                message_count: row.try_get::<i64, _>("message_count")? as u32,
            });
        }

        Ok(summaries)
    }

    /// Exclui uma conversa e todas as suas mensagens.
    pub async fn delete(&self, conversation_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM conversations WHERE id = ?1")
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;

        info!(conversation_id = %conversation_id, "Conversa excluída");
        Ok(())
    }

    /// Atualiza o título de uma conversa.
    pub async fn set_title(
        &self,
        conversation_id: &str,
        title: impl Into<String>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE conversations
            SET title = ?1, updated_at = unixepoch()
            WHERE id = ?2
            "#,
        )
        .bind(title.into())
        .bind(conversation_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct ConversationSummary {
    pub id: String,
    pub title: Option<String>,
    pub provider: String,
    pub model: String,
    pub created_at: i64,
    pub message_count: u32,
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use hyscode_core::models::message::MessageContent;

    async fn setup_manager() -> ConversationManager {
        // Usa banco em memória para testes — mais confiável e rápido.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        ConversationManager::run_migrations(&pool).await.unwrap();

        ConversationManager { pool }
    }

    #[tokio::test]
    async fn test_create_conversation() {
        let manager = setup_manager().await;
        let id = manager.create("openai", "gpt-4o").await.unwrap();
        assert!(!id.is_empty());

        let recent = manager.list_recent(10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].provider, "openai");
        assert_eq!(recent[0].model, "gpt-4o");
    }

    #[tokio::test]
    async fn test_add_and_load_messages() {
        let manager = setup_manager().await;
        let id = manager
            .create("anthropic", "claude-3-5-sonnet")
            .await
            .unwrap();

        manager
            .add_message(
                &id,
                &Message::System {
                    content: "Você é um assistente.".to_owned(),
                },
            )
            .await
            .unwrap();

        manager
            .add_message(
                &id,
                &Message::User {
                    content: MessageContent::Text("Olá".to_owned()),
                },
            )
            .await
            .unwrap();

        manager
            .add_message(
                &id,
                &Message::Assistant {
                    content: Some("Olá!".to_owned()),
                    tool_calls: None,
                    thinking: None,
                },
            )
            .await
            .unwrap();

        let messages = manager.load_messages(&id).await.unwrap();
        assert_eq!(messages.len(), 3);

        match &messages[0] {
            Message::System { content } => assert_eq!(content, "Você é um assistente."),
            _ => panic!("Esperado System message"),
        }

        match &messages[1] {
            Message::User { content } => match content {
                MessageContent::Text(t) => assert_eq!(t, "Olá"),
                _ => panic!("Esperado Text content"),
            },
            _ => panic!("Esperado User message"),
        }

        match &messages[2] {
            Message::Assistant { content, .. } => {
                assert_eq!(content, &Some("Olá!".to_owned()));
            }
            _ => panic!("Esperado Assistant message"),
        }
    }

    #[tokio::test]
    async fn test_delete_conversation() {
        let manager = setup_manager().await;
        let id = manager.create("openai", "gpt-4o").await.unwrap();

        manager
            .add_message(
                &id,
                &Message::User {
                    content: MessageContent::Text("test".to_owned()),
                },
            )
            .await
            .unwrap();

        manager.delete(&id).await.unwrap();

        let messages = manager.load_messages(&id).await.unwrap();
        assert!(messages.is_empty());

        let recent = manager.list_recent(10).await.unwrap();
        assert!(recent.is_empty());
    }

    #[tokio::test]
    async fn test_set_title() {
        let manager = setup_manager().await;
        let id = manager.create("openai", "gpt-4o").await.unwrap();

        manager.set_title(&id, "Minha Conversa").await.unwrap();

        let recent = manager.list_recent(10).await.unwrap();
        assert_eq!(recent[0].title, Some("Minha Conversa".to_owned()));
    }

    #[tokio::test]
    async fn test_list_recent_order() {
        let manager = setup_manager().await;
        let id1 = manager.create("openai", "gpt-4o").await.unwrap();

        // Delay para garantir timestamp diferente no SQLite (unixepoch = segundos)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let id2 = manager.create("anthropic", "claude-3").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Adiciona mensagem na conversa 1 para atualizar updated_at
        manager
            .add_message(
                &id1,
                &Message::User {
                    content: MessageContent::Text("msg".to_owned()),
                },
            )
            .await
            .unwrap();

        let recent = manager.list_recent(10).await.unwrap();
        assert_eq!(recent.len(), 2);
        // A conversa 1 deve vir primeiro (foi atualizada depois)
        assert_eq!(recent[0].id, id1);
        assert_eq!(recent[1].id, id2);
    }
}
