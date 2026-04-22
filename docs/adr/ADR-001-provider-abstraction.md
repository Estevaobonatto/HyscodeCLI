# ADR-001: Abstração de Provedores (Provider Adapter Pattern)

> **Status:** Aceito  
> **Data:** 2026-04-22  
> **Decisores:** Arquiteto, Tech Lead  

## Contexto

O HyscodeCLI precisa se conectar a múltiplos provedores de LLM (Anthropic, OpenAI, GitHub Copilot, Z.ai, OpenRouter, e nosso próprio Hyscode Provider). Cada provedor possui:

- APIs diferentes (REST, SSE, WebSocket)
- Formatos de request/response distintos
- Esquemas de autenticação variados
- Capacidades distintas (tool calling, vision, streaming)
- Modelos com diferentes limites de contexto e custos

Precisamos de uma abstração que permita:
1. Adicionar novos provedores com mínimo impacto no código existente
2. Trocar de provedor em runtime sem mudanças na lógica do engine
3. Suportar provedores que ainda não existem

## Decisão

Adotaremos o **Adapter Pattern** combinado com **Strategy Pattern** para abstração de provedores.

### Interface Principal (Port)

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> Result<BoxStream<'_, ChatChunk>>;
    
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
    async fn validate(&self) -> Result<()>;
    
    fn estimate_tokens(&self, messages: &[Message]) -> u32;
}
```

### Estrutura de Adapters

```
hyscode-provider/
├── src/
│   ├── lib.rs              # Provider trait + registro
│   ├── registry.rs         # ProviderRegistry (factory)
│   ├── models.rs           # Tipos comuns (ChatRequest, Message, etc)
│   ├── adapters/
│   │   ├── mod.rs
│   │   ├── openai.rs       # OpenAIAdapter
│   │   ├── anthropic.rs    # AnthropicAdapter
│   │   ├── copilot.rs      # GitHubCopilotAdapter
│   │   ├── openrouter.rs   # OpenRouterAdapter
│   │   ├── zai.rs          # ZAiAdapter
│   │   └── hyscode.rs      # HyscodeProviderAdapter
│   └── http/
│       ├── client.rs       # HTTP client compartilhado
│       └── sse.rs          # Parser de SSE
```

### Normalização de Request/Response

Cada adapter converte de/para um formato canônico interno:

**Formato Canônico:**
```rust
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

pub enum Message {
    System { content: String },
    User { content: Content },
    Assistant { content: Option<String>, tool_calls: Option<Vec<ToolCall>> },
    Tool { tool_call_id: String, content: String },
}
```

**Conversão OpenAI:**
```rust
impl OpenAIAdapter {
    fn to_openai_request(&self, req: ChatRequest) -> OpenAIChatRequest {
        // Converte Message::System → system role
        // Converte Message::User → user role
        // Converte ToolDefinition → OpenAI function schema
    }
}
```

**Conversão Anthropic:**
```rust
impl AnthropicAdapter {
    fn to_anthropic_request(&self, req: ChatRequest) -> AnthropicRequest {
        // Converte para Messages API format
        // System prompt vai em campo separado
        // Tool use → tool_use blocks
    }
}
```

## Consequências

### Positivas
- **Extensibilidade:** Novo provedor = novo arquivo adapter
- **Testabilidade:** Podemos mockar o trait `Provider` em testes
- **Consistência:** Engine trabalha com um formato único
- **Feature flags:** Cada adapter pode ser feature opcional no Cargo.toml

### Negativas
- **Overhead de conversão:** Cada request/response passa por uma camada de tradução
- **Latência adicional:** Conversão síncrona rápida, mas existe
- **Complexidade:** Precisamos manter mapeamento de capacidades (quais modelos suportam tools, vision, etc)

## Alternativas Consideradas

| Alternativa | Descrição | Motivo do descarte |
|-------------|-----------|-------------------|
| Usar SDKs oficiais | Usar crates oficiais de cada provedor | Nem todos têm SDK Rust; adicionam dependências pesadas; APIs mudam frequentemente |
| OpenRouter apenas | Delegar tudo ao OpenRouter | Não cobre todos os casos; perde controle sobre autenticação; custo adicional |
| Formato OpenAI universal | Forçar todos a parecer OpenAI | Anthropic tem features únicas (computer use, artifacts) que perderíamos |

## Notas de Implementação

1. **HTTP Client compartilhado:** Todos os adapters usam um `reqwest::Client` único com pool de conexões
2. **SSE unificado:** Parser de SSE genérico que normaliza eventos de diferentes provedores
3. **Capacidades declarativas:** Cada adapter retorna `ProviderCapabilities { tools: bool, vision: bool, max_context: u32 }`
4. **Erros normalizados:** `ProviderError` enum com variantes comuns (RateLimited, InvalidKey, Timeout, etc)
