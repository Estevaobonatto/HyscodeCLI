//! Contagem de tokens e estimativa de custo.

use std::ops::Add;

/// Contagem de tokens de uma requisição/resposta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    pub fn new(prompt: u32, completion: u32) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
        }
    }
}

impl Add for TokenUsage {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(
            self.prompt_tokens + rhs.prompt_tokens,
            self.completion_tokens + rhs.completion_tokens,
        )
    }
}
