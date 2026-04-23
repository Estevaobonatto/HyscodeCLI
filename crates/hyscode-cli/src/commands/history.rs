//! Comando `hyscode history` — lista conversas recentes.

use hyscode_engine::conversation::ConversationManager;

/// Caminho padrão do banco de conversas (igual ao do chat.rs).
fn conversations_db_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hyscode")
        .join("conversations.db")
}

pub async fn run(limit: usize) -> anyhow::Result<()> {
    let db_path = conversations_db_path();

    if !db_path.exists() {
        println!("Nenhuma conversa encontrada.");
        return Ok(());
    }

    let manager = ConversationManager::new(db_path).await?;
    let conversations = manager.list_recent(limit as u32).await?;

    if conversations.is_empty() {
        println!("Nenhuma conversa encontrada.");
        return Ok(());
    }

    println!(
        "{:<26}  {:<12}  {:<20}  {:>5}  {}",
        "ID", "Provedor", "Modelo", "Msgs", "Criado em"
    );
    println!("{}", "─".repeat(85));

    for conv in conversations {
        let created = chrono::DateTime::from_timestamp(conv.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "—".to_owned());

        let title_or_id = conv
            .title
            .as_deref()
            .unwrap_or(&conv.id)
            .chars()
            .take(26)
            .collect::<String>();

        println!(
            "{:<26}  {:<12}  {:<20}  {:>5}  {}",
            title_or_id,
            conv.provider.chars().take(12).collect::<String>(),
            conv.model.chars().take(20).collect::<String>(),
            conv.message_count,
            created
        );
    }

    Ok(())
}
