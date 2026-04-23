//! Parser de Server-Sent Events (SSE) para streaming de respostas.
//!
//! Normaliza o formato de eventos de diferentes provedores:
//! - OpenAI: `data: {...}\n\n`, termina com `data: [DONE]`
//! - Anthropic: eventos com campo `event:` antes do `data:`

use hyscode_core::models::enums::{SSE_DATA_PREFIX, SSE_DONE_SENTINEL};

/// Evento SSE bruto, antes de parse do JSON.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub data: String,
}

impl SseEvent {
    /// Retorna true se este evento indica fim do stream.
    pub fn is_done(&self) -> bool {
        self.data.trim() == SSE_DONE_SENTINEL
    }
}

/// Parseia uma linha de SSE em um evento, se aplicável.
pub fn parse_sse_line(line: &str) -> Option<SseEvent> {
    line.strip_prefix(SSE_DATA_PREFIX).map(|data| SseEvent {
        event_type: None,
        data: data.to_owned(),
    })
}

/// Parseia um bloco completo de SSE (pode conter event: e data:).
pub fn parse_sse_block(block: &str) -> Option<SseEvent> {
    let mut event_type = None;
    let mut data = None;

    for line in block.lines() {
        if let Some(et) = line.strip_prefix("event: ") {
            event_type = Some(et.to_owned());
        } else if let Some(d) = line.strip_prefix(SSE_DATA_PREFIX) {
            data = Some(d.to_owned());
        }
    }

    data.map(|d| SseEvent {
        event_type,
        data: d,
    })
}
