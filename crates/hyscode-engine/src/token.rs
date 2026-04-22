//! Estimativa de tokens e controle de limites de contexto.

/// Estima e controla o uso de tokens nas requisições.
pub struct TokenEstimator {
    pub model_context_limit: u32,
    pub reserved_for_completion: u32,
}

impl TokenEstimator {
    pub fn new(model_context_limit: u32) -> Self {
        Self {
            model_context_limit,
            // Reserva 20% do contexto para a resposta
            reserved_for_completion: model_context_limit / 5,
        }
    }

    /// Estima tokens de um texto (heurística: ~4 chars por token).
    pub fn estimate(&self, text: &str) -> u32 {
        (text.len() as u32).saturating_div(4).max(1)
    }

    /// Tokens disponíveis para o prompt (contexto - reserva).
    pub fn available_for_prompt(&self) -> u32 {
        self.model_context_limit
            .saturating_sub(self.reserved_for_completion)
    }

    /// Verifica se o prompt cabe dentro do limite.
    pub fn fits(&self, prompt_tokens: u32) -> bool {
        prompt_tokens <= self.available_for_prompt()
    }

    /// Trunca uma lista de mensagens para caber no limite,
    /// preservando sempre a primeira (system) e as últimas mensagens.
    pub fn truncate_messages<T: Clone>(
        &self,
        messages: &[T],
        token_fn: impl Fn(&T) -> u32,
    ) -> Vec<T> {
        let limit = self.available_for_prompt();
        let mut total = 0u32;
        let mut result: Vec<T> = Vec::new();

        // Sempre inclui a primeira mensagem (system prompt)
        if let Some(first) = messages.first() {
            total += token_fn(first);
            result.push(first.clone());
        }

        // Inclui mensagens do fim para o início até atingir o limite
        let rest: Vec<&T> = messages.iter().skip(1).collect();
        let mut tail: Vec<T> = Vec::new();

        for msg in rest.iter().rev() {
            let tokens = token_fn(msg);
            if total + tokens > limit {
                break;
            }
            total += tokens;
            tail.push((*msg).clone());
        }

        tail.reverse();
        result.extend(tail);
        result
    }
}
