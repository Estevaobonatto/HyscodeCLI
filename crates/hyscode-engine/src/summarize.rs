//! Auto-sumarização de contexto quando tokens excedem 85% do limite.
//!
//! Quando a janela de contexto está quase cheia, as mensagens mais antigas
//! (exceto o system prompt) são condensadas em um único resumo pelo LLM,
//! reduzindo o tamanho do histórico sem perder o contexto essencial.

use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;
use hyscode_core::{
    models::{
        message::{Message, MessageContent},
        request::ChatRequest,
    },
    traits::provider::Provider,
};
use tracing::info;

use crate::token::TokenEstimator;

/// Limiar padrão (85%) acima do qual a sumarização é acionada.
const SUMMARIZE_THRESHOLD: f32 = 0.85;

/// System prompt para o LLM que faz o resumo.
const SUMMARY_SYSTEM_PROMPT: &str =
    "You are a conversation summarizer. Given a conversation history, produce a concise \
     but complete summary that captures all key decisions, facts, file changes, \
     and context needed to continue the conversation. \
     Respond with the summary only — no preamble.";

/// Extrai o conteúdo textual de uma mensagem para estimativa de tokens.
fn message_text(msg: &Message) -> String {
    match msg {
        Message::System { content } => content.clone(),
        Message::User { content } => match content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    hyscode_core::models::message::ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        },
        Message::Assistant { content, .. } => content.clone().unwrap_or_default(),
        Message::Tool { content, .. } => content.clone(),
    }
}

/// Verifica se a conversa precisa de sumarização e, em caso afirmativo,
/// retorna uma lista de mensagens enxuta (system + resumo + mensagens recentes).
///
/// # Arguments
/// * `messages` — lista atual de mensagens
/// * `provider` — provedor LLM para gerar o resumo
/// * `model` — nome do modelo a usar para o resumo
/// * `estimator` — estimador de tokens para o modelo em uso
///
/// # Returns
/// `Some(condensed_messages)` se houve sumarização, `None` se não necessário.
pub async fn maybe_summarize(
    messages: &[Message],
    provider: Arc<dyn Provider>,
    model: &str,
    estimator: &TokenEstimator,
) -> Result<Option<Vec<Message>>> {
    if messages.is_empty() {
        return Ok(None);
    }

    // Calcula tokens totais da conversa atual
    let total_tokens: u32 = messages.iter().map(|m| estimator.estimate(&message_text(m))).sum();
    let threshold = (estimator.model_context_limit as f32 * SUMMARIZE_THRESHOLD) as u32;

    if total_tokens <= threshold {
        return Ok(None);
    }

    info!(
        total_tokens,
        threshold,
        "Contexto excede {}% do limite — sumarizando",
        (SUMMARIZE_THRESHOLD * 100.0) as u32
    );

    // Separa system prompt e histórico de conversação
    let (system_msgs, history): (Vec<&Message>, Vec<&Message>) =
        messages.iter().partition(|m| matches!(m, Message::System { .. }));

    // Mantém as últimas N mensagens fora do resumo para continuidade
    let keep_recent = 6usize.min(history.len());
    let (to_summarize, recent) = history.split_at(history.len().saturating_sub(keep_recent));

    if to_summarize.is_empty() {
        return Ok(None);
    }

    // Constrói o texto a sumarizar
    let conversation_text = to_summarize
        .iter()
        .map(|m| {
            let role = match m {
                Message::User { .. } => "user",
                Message::Assistant { .. } => "assistant",
                Message::Tool { .. } => "tool",
                Message::System { .. } => "system",
            };
            format!("{}: {}", role, message_text(m))
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Chama o LLM para gerar o resumo
    let summary = call_summary_llm(provider, model, &conversation_text).await?;

    // Reconstrói a lista de mensagens
    let mut condensed: Vec<Message> = system_msgs.into_iter().cloned().collect();
    condensed.push(Message::Assistant {
        content: Some(format!("[Resumo da conversa anterior]\n{}", summary)),
        tool_calls: None,
        thinking: None,
    });
    condensed.extend(recent.iter().map(|m| (*m).clone()));

    info!(
        original = messages.len(),
        condensed = condensed.len(),
        "Conversa resumida"
    );

    Ok(Some(condensed))
}

/// Chama o LLM para gerar um resumo do texto fornecido.
async fn call_summary_llm(
    provider: Arc<dyn Provider>,
    model: &str,
    conversation_text: &str,
) -> Result<String> {
    let request = ChatRequest::new(
        model,
        vec![
            Message::System {
                content: SUMMARY_SYSTEM_PROMPT.to_owned(),
            },
            Message::User {
                content: MessageContent::Text(format!(
                    "Summarize this conversation:\n\n{}",
                    conversation_text
                )),
            },
        ],
    )
    .with_max_tokens(1024)
    .with_temperature(0.3)
    .with_stream();

    let mut response_text = String::new();

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                if let Ok(chunk) = chunk_result {
                    if let Some(content) = chunk.delta.content {
                        response_text.push_str(&content);
                    }
                }
            }
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Falha ao gerar resumo: {}", e));
        }
    }

    Ok(response_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_calculation() {
        let estimator = TokenEstimator::new(4096);
        let threshold = (estimator.model_context_limit as f32 * SUMMARIZE_THRESHOLD) as u32;
        assert_eq!(threshold, 3481);
    }
}
