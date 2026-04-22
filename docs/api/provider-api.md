# API Contract — Hyscode Provider Service

> **Versão:** v1.0.0  
> **Base URL:** `https://api.hyscode.dev/v1`  
> **Autenticação:** Bearer Token (`Authorization: Bearer hsk_...`)  
> **Formato:** JSON  
> **Streaming:** Server-Sent Events (SSE)

---

## Autenticação

Todas as requisições devem incluir o header:

```http
Authorization: Bearer hsk_<chave-api>
```

Erros de autenticação:

| Código | Mensagem | Causa |
|--------|----------|-------|
| 401 | `invalid_api_key` | Chave inexistente ou inválida |
| 401 | `api_key_revoked` | Chave foi revogada |
| 403 | `plan_limit_exceeded` | Plano não suporta este modelo |
| 429 | `rate_limit_exceeded` | Muitas requisições |

---

## 1. Chat Completions

### `POST /v1/chat/completions`

Cria uma resposta do modelo para uma conversa. Compatível com OpenAI Chat API.

**Request Headers:**
```http
Content-Type: application/json
Authorization: Bearer hsk_...
```

**Request Body:**
```json
{
  "model": "hyscode-smart",
  "messages": [
    {
      "role": "system",
      "content": "Você é um assistente de codificação especializado em Rust."
    },
    {
      "role": "user",
      "content": "Explique o que é ownership em Rust."
    }
  ],
  "stream": false,
  "temperature": 0.7,
  "max_tokens": 2048,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "read_file",
        "description": "Lê o conteúdo de um arquivo",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Caminho do arquivo"
            }
          },
          "required": ["path"]
        }
      }
    }
  ],
  "tool_choice": "auto"
}
```

**Campos do Request:**

| Campo | Tipo | Obrigatório | Descrição |
|-------|------|-------------|-----------|
| `model` | string | Sim | ID do modelo (ver `/v1/models`) |
| `messages` | array | Sim | Histórico de mensagens |
| `stream` | boolean | Não | Se `true`, retorna SSE. Default: `false` |
| `temperature` | float | Não | 0.0 - 2.0. Default: 1.0 |
| `max_tokens` | integer | Não | Máximo de tokens na resposta |
| `tools` | array | Não | Ferramentas disponíveis para o modelo |
| `tool_choice` | string/object | Não | `auto`, `none`, ou `{"type":"function","function":{"name":"..."}}` |
| `top_p` | float | Não | Nucleus sampling |
| `stop` | string/array | Não | Sequências de parada |
| `user` | string | Não | ID do usuário final (para monitoramento) |

**Response (stream: false):**
```json
{
  "id": "chatcmpl-hsc-8f4a3c2e",
  "object": "chat.completion",
  "created": 1713792000,
  "model": "hyscode-smart",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Ownership é o mecanismo central de gerenciamento de memória em Rust...",
        "tool_calls": null
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 45,
    "completion_tokens": 312,
    "total_tokens": 357
  },
  "x-hyscode": {
    "provider": "anthropic",
    "model_upstream": "claude-3-5-sonnet-20241022",
    "latency_ms": 1243,
    "cost_usd": 0.00089
  }
}
```

**Response com tool_calls:**
```json
{
  "id": "chatcmpl-hsc-9a1b2d3f",
  "object": "chat.completion",
  "created": 1713792001,
  "model": "hyscode-smart",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          {
            "id": "call_abc123",
            "type": "function",
            "function": {
              "name": "read_file",
              "arguments": "{\"path\": \"src/main.rs\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ],
  "usage": {
    "prompt_tokens": 87,
    "completion_tokens": 23,
    "total_tokens": 110
  }
}
```

**Response (stream: true) — Server-Sent Events:**
```
data: {"id":"chatcmpl-hsc-8f4a3c2e","object":"chat.completion.chunk","created":1713792000,"model":"hyscode-smart","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-hsc-8f4a3c2e","object":"chat.completion.chunk","created":1713792000,"model":"hyscode-smart","choices":[{"index":0,"delta":{"content":"Ownership"},"finish_reason":null}]}

data: {"id":"chatcmpl-hsc-8f4a3c2e","object":"chat.completion.chunk","created":1713792000,"model":"hyscode-smart","choices":[{"index":0,"delta":{"content":" é"},"finish_reason":null}]}

data: {"id":"chatcmpl-hsc-8f4a3c2e","object":"chat.completion.chunk","created":1713792001,"model":"hyscode-smart","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":45,"completion_tokens":312,"total_tokens":357}}

data: [DONE]
```

---

## 2. Modelos

### `GET /v1/models`

Lista todos os modelos disponíveis para a conta.

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "hyscode-fast",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "description": "Rápido e econômico. Ideal para tarefas simples.",
      "context_window": 200000,
      "supports_tools": true,
      "supports_vision": false,
      "tier_required": "free",
      "x-hyscode": {
        "provider": "anthropic",
        "model_upstream": "claude-3-haiku-20240307",
        "price_per_1k_input": 0.00025,
        "price_per_1k_output": 0.00125
      }
    },
    {
      "id": "hyscode-smart",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "description": "Melhor custo-benefício para coding.",
      "context_window": 200000,
      "supports_tools": true,
      "supports_vision": true,
      "tier_required": "free",
      "x-hyscode": {
        "provider": "anthropic",
        "model_upstream": "claude-3-5-sonnet-20241022",
        "price_per_1k_input": 0.003,
        "price_per_1k_output": 0.015
      }
    },
    {
      "id": "hyscode-ultra",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "description": "Máxima capacidade de raciocínio.",
      "context_window": 128000,
      "supports_tools": true,
      "supports_vision": true,
      "tier_required": "pro",
      "x-hyscode": {
        "provider": "openai",
        "model_upstream": "gpt-4o",
        "price_per_1k_input": 0.005,
        "price_per_1k_output": 0.015
      }
    },
    {
      "id": "hyscode-code",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "description": "Especializado em codificação.",
      "context_window": 200000,
      "supports_tools": true,
      "supports_vision": false,
      "tier_required": "pro",
      "x-hyscode": {
        "provider": "anthropic",
        "model_upstream": "claude-3-5-sonnet-20241022",
        "price_per_1k_input": 0.003,
        "price_per_1k_output": 0.015
      }
    }
  ]
}
```

### `GET /v1/models/{model_id}`

Retorna detalhes de um modelo específico.

---

## 3. Uso e Billing

### `GET /v1/usage`

Retorna dados de uso da conta.

**Query Parameters:**

| Parâmetro | Tipo | Descrição |
|-----------|------|-----------|
| `start_date` | string (ISO 8601) | Início do período |
| `end_date` | string (ISO 8601) | Fim do período |
| `granularity` | string | `day`, `week`, `month`. Default: `day` |
| `model` | string | Filtrar por modelo |

**Response:**
```json
{
  "object": "usage.summary",
  "period": {
    "start": "2026-04-01T00:00:00Z",
    "end": "2026-04-22T23:59:59Z"
  },
  "totals": {
    "requests": 1423,
    "input_tokens": 2847392,
    "output_tokens": 934821,
    "total_tokens": 3782213,
    "cost_usd": 12.47
  },
  "by_model": [
    {
      "model": "hyscode-smart",
      "requests": 987,
      "input_tokens": 1943021,
      "output_tokens": 721034,
      "cost_usd": 9.23
    },
    {
      "model": "hyscode-fast",
      "requests": 436,
      "input_tokens": 904371,
      "output_tokens": 213787,
      "cost_usd": 3.24
    }
  ],
  "daily": [
    {
      "date": "2026-04-22",
      "requests": 87,
      "total_tokens": 234821,
      "cost_usd": 0.82
    }
  ]
}
```

---

## 4. API Keys

### `GET /v1/api-keys`

Lista as API keys da conta.

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "apikey_a1b2c3",
      "name": "Producao CLI",
      "prefix": "hsk_prod_",
      "created_at": "2026-03-15T10:00:00Z",
      "last_used_at": "2026-04-22T16:00:00Z",
      "revoked": false,
      "permissions": ["chat", "models", "usage"]
    }
  ]
}
```

### `POST /v1/api-keys`

Cria uma nova API key.

**Request:**
```json
{
  "name": "Minha nova chave",
  "permissions": ["chat", "models"]
}
```

**Response:**
```json
{
  "id": "apikey_d4e5f6",
  "name": "Minha nova chave",
  "key": "hsk_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "created_at": "2026-04-22T16:30:00Z",
  "permissions": ["chat", "models"],
  "note": "Guarde esta chave. Ela não será exibida novamente."
}
```

### `DELETE /v1/api-keys/{key_id}`

Revoga uma API key imediatamente.

**Response:**
```json
{
  "id": "apikey_d4e5f6",
  "revoked": true,
  "revoked_at": "2026-04-22T17:00:00Z"
}
```

---

## 5. Conta e Plano

### `GET /v1/account`

Retorna dados da conta e plano atual.

**Response:**
```json
{
  "id": "acct_xyz123",
  "email": "dev@empresa.com",
  "name": "João Silva",
  "plan": {
    "id": "pro",
    "name": "Pro",
    "status": "active",
    "current_period_start": "2026-04-01T00:00:00Z",
    "current_period_end": "2026-05-01T00:00:00Z",
    "price_usd": 19.00,
    "limits": {
      "requests_per_minute": 60,
      "requests_per_day": 10000,
      "tokens_per_month": null,
      "models": ["hyscode-fast", "hyscode-smart", "hyscode-ultra", "hyscode-code"]
    }
  },
  "billing": {
    "balance_usd": 0.00,
    "next_invoice_date": "2026-05-01T00:00:00Z",
    "next_invoice_amount_usd": 19.00
  }
}
```

---

## 6. Erros

### Formato Padrão de Erro

```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Limite de requisições excedido. Tente novamente em 30 segundos.",
    "type": "rate_limit_error",
    "param": null,
    "retry_after": 30
  }
}
```

### Códigos de Erro

| HTTP | Código | Descrição |
|------|--------|-----------|
| 400 | `invalid_request` | Request malformado |
| 400 | `invalid_model` | Modelo não existe |
| 400 | `context_length_exceeded` | Prompt excede limite do modelo |
| 401 | `invalid_api_key` | Chave inválida |
| 401 | `api_key_revoked` | Chave revogada |
| 403 | `plan_limit_exceeded` | Modelo não disponível no plano |
| 429 | `rate_limit_exceeded` | Muitas requisições |
| 500 | `internal_error` | Erro interno do serviço |
| 502 | `upstream_error` | Erro no provedor upstream |
| 503 | `service_unavailable` | Serviço temporariamente indisponível |
| 504 | `upstream_timeout` | Timeout no provedor upstream |

---

## 7. Headers de Resposta

| Header | Descrição |
|--------|-----------|
| `X-Request-Id` | ID único da requisição (para suporte) |
| `X-RateLimit-Limit` | Limite de requisições por minuto |
| `X-RateLimit-Remaining` | Requisições restantes no período |
| `X-RateLimit-Reset` | Unix timestamp do reset |
| `X-Hyscode-Provider` | Provedor upstream utilizado |
| `X-Hyscode-Latency-Ms` | Latência do provedor upstream |

---

## 8. Webhooks (Futuro)

**Eventos disponíveis:**

| Evento | Descrição |
|--------|-----------|
| `usage.limit.warning` | Atingiu 80% do limite mensal |
| `usage.limit.reached` | Atingiu 100% do limite |
| `payment.failed` | Falha na cobrança |
| `api_key.created` | Nova chave criada |
| `api_key.revoked` | Chave revogada |

**Payload:**
```json
{
  "event": "usage.limit.warning",
  "timestamp": "2026-04-22T16:00:00Z",
  "data": {
    "account_id": "acct_xyz123",
    "usage_percent": 80,
    "tokens_used": 8000000,
    "tokens_limit": 10000000
  }
}
```
