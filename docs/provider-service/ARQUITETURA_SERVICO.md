# Arquitetura do Provider Service — hyscode-gateway

> **Versão:** 0.1.0  
> **Fase:** 4 (implementação inicial)  
> **Stack:** Rust · Axum · PostgreSQL · Redis  
> **Última atualização:** 2026-04-22

---

## 1. Visão Geral

O **hyscode-gateway** é um proxy HTTP autenticado que expõe uma API compatível com
OpenAI e encaminha as requisições para provedores LLM upstream (Anthropic, OpenAI, etc.).

Responsabilidades principais:
- Autenticar requisições via Bearer token (`hsk_...`)
- Validar e rotear para o provedor upstream correto conforme o modelo solicitado
- Registrar uso (tokens, latência, status) no PostgreSQL
- Controlar quotas mensais de tokens por usuário
- Fazer proxy de respostas completas e streaming (SSE)

---

## 2. Diagrama de Componentes

```text
                     Internet
  CLI / Cliente ──────────────► hyscode-gateway (Axum, porta 3000)
                                        │
                  ┌─────────────────────┼──────────────────────┐
                  ▼                     ▼                       ▼
           auth_middleware        router (axum)           upstream proxy
           (Bearer → SHA-256)     /v1/chat/completions    (reqwest + SSE)
           (JWT → usuário)        /v1/models                    │
                  │               /health                       ▼
                  │               /auth/*                    Anthropic /
                  │               /apikeys                     OpenAI /
                  │               /dashboard/*                 Groq / ...
                  │               /billing/*
                  │               /admin/*
                  ▼
            PostgreSQL DB
            (api_keys, users, requests_log,
             usage_quotas, subscription_plans,
             subscriptions, billing_records,
             pricing_per_model, provider_health)
                  │
                  ▼
               Redis
          (rate limiting, api_key cache)
```

---

## 3. Módulos do Crate (`hyscode-gateway`)

| Módulo | Arquivo | Responsabilidade |
|--------|---------|-----------------|
| `main` | `main.rs` | Bootstrap: carrega config, conecta DB/Redis, inicia servidor |
| `config` | `config.rs` | Lê variáveis de ambiente; define `Config` e `ModelRoute` |
| `router` | `router.rs` | Monta o `Router` axum com middlewares e todas as rotas |
| `auth` | `auth.rs` | Middleware Bearer token (SHA-256) e JWT; verificação de role admin |
| `upstream` | `upstream.rs` | Handlers das rotas; encaminha para provedor upstream |
| `models` | `models.rs` | DTOs de request/response da API |
| `db` | `db.rs` | Funções de acesso ao PostgreSQL e Redis |
| `error` | `error.rs` | `GatewayError` com conversão automática para resposta HTTP |
| `users` | `users.rs` | Registro e login de usuários com JWT (Argon2) |
| `apikeys` | `apikeys.rs` | CRUD de API keys para usuários autenticados |
| `billing` | `billing.rs` | Cálculo de custo por request, alertas de quota, planos |
| `dashboard` | `dashboard.rs` | Métricas de uso por usuário e por modelo |
| `admin` | `admin.rs` | Gestão de preços, health de provedores e métricas de negócio |

---

## 4. Rotas HTTP

### Públicas

| Método | Path | Auth | Descrição |
|--------|------|------|-----------|
| `GET`  | `/health` | ❌ | Health check |
| `POST` | `/auth/register` | ❌ | Cria conta de usuário |
| `POST` | `/auth/login` | ❌ | Autentica e retorna JWT |

### OpenAI-compatible (API Key)

| Método | Path | Auth | Descrição |
|--------|------|------|-----------|
| `GET`  | `/v1/models` | ✅ Bearer `hsk_` | Lista modelos disponíveis |
| `POST` | `/v1/chat/completions` | ✅ Bearer `hsk_` | Chat completion (SSE suportado) |

### Usuário (JWT)

| Método | Path | Auth | Descrição |
|--------|------|------|-----------|
| `GET`  | `/apikeys` | ✅ JWT | Lista API keys do usuário |
| `POST` | `/apikeys` | ✅ JWT | Cria nova API key |
| `DELETE`| `/apikeys/:id` | ✅ JWT | Revoga API key |
| `GET`  | `/dashboard/usage` | ✅ JWT | Resumo de uso e quota |
| `GET`  | `/dashboard/usage/by-model` | ✅ JWT | Uso agrupado por modelo |
| `GET`  | `/billing/plans` | ✅ JWT | Lista planos de assinatura |
| `GET`  | `/billing/records` | ✅ JWT | Histórico de billing |
| `GET`  | `/billing/alerts` | ✅ JWT | Alertas de quota |
| `POST` | `/billing/alerts/:id/read` | ✅ JWT | Marca alerta como lido |

### Admin (JWT + role admin)

| Método | Path | Auth | Descrição |
|--------|------|------|-----------|
| `GET`  | `/admin/pricing` | ✅ JWT admin | Lista preços por modelo |
| `POST` | `/admin/pricing` | ✅ JWT admin | Define preço para modelo |
| `PUT`  | `/admin/pricing/:id` | ✅ JWT admin | Atualiza preço |
| `GET`  | `/admin/provider-health` | ✅ JWT admin | Histórico de saúde dos provedores |
| `POST` | `/admin/provider-health/check` | ✅ JWT admin | Executa health check manual |
| `GET`  | `/admin/metrics` | ✅ JWT admin | Métricas de negócio (7 dias) |

---

## 5. Autenticação

1. Cliente envia `Authorization: Bearer hsk_<token>`
2. Middleware extrai o token e calcula `SHA-256(token)`
3. Busca em `api_keys.key_hash` no PostgreSQL
4. Verifica `is_active = TRUE` e `expires_at` (se definido)
5. Injeta `AuthContext` (user_id, api_key_id, scopes, rate_limit_rpm) na requisição
6. Rotas protegidas acessam `AuthContext` via `Extension<AuthContext>`

```rust
pub struct AuthContext {
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    pub scopes: Vec<String>,    // ["chat", "models"]
    pub rate_limit_rpm: i32,
}
```

---

## 6. Roteamento de Modelos (Model Routes)

Configuração via variável de ambiente `MODEL_ROUTES`:

```
MODEL_ROUTES=claude-3-5-sonnet=anthropic,gpt-4o=openai,gpt-4o-mini=openai
```

Formato: `alias=provider,...`

O `AppState::resolve_provider(model)` procura o alias na lista e retorna o `ModelRoute`.
Cada `ModelRoute` tem `model` (alias externo) e `provider` (nome do provedor upstream).

Os endpoints dos provedores upstream e suas API keys são lidos de variáveis de ambiente:
- `ANTHROPIC_API_KEY` → `https://api.anthropic.com/v1`
- `OPENAI_API_KEY` → `https://api.openai.com/v1`

---

## 7. Streaming (SSE)

Quando `stream: true` na requisição:
1. Handler chama o provedor upstream com `stream: true`
2. Resposta upstream (SSE chunks) é lida via `reqwest::Response::bytes_stream()`
3. Cada chunk é repassado como `Event::default().data(chunk)` via `axum::response::sse::Sse`
4. Keep-alive automático (`KeepAlive::default()`) mantém conexão em requisições longas

---

## 8. Configuração (Variáveis de Ambiente)

| Variável | Obrigatória | Padrão | Descrição |
|----------|-------------|--------|-----------|
| `DATABASE_URL` | ✅ | — | URL PostgreSQL (`postgres://user:pass@host/db`) |
| `REDIS_URL` | ✅ | — | URL Redis (`redis://host:6379`) |
| `JWT_SECRET` | ✅ | — | Segredo para assinar JWTs internos |
| `LISTEN_ADDR` | ❌ | `0.0.0.0:3000` | Endereço de escuta do servidor |
| `MODEL_ROUTES` | ❌ | `""` | Mapeamento `alias=provider,...` |
| `ANTHROPIC_API_KEY` | ❌ | — | API key para Anthropic upstream |
| `OPENAI_API_KEY` | ❌ | — | API key para OpenAI upstream |
| `LOG_LEVEL` | ❌ | `info` | Nível de log (`trace`, `debug`, `info`, `warn`, `error`) |

---

## 9. Erros da API

Todos os erros são retornados como JSON:

```json
{
  "error": {
    "code": "unauthorized",
    "message": "Não autenticado"
  }
}
```

| `GatewayError` | HTTP Status | `code` |
|----------------|-------------|--------|
| `Unauthorized` | 401 | `unauthorized` |
| `Forbidden` | 403 | `forbidden` |
| `QuotaExceeded` | 429 | `quota_exceeded` |
| `ModelNotFound` | 404 | `model_not_found` |
| `NotFound` | 404 | `not_found` |
| `Conflict` | 409 | `conflict` |
| `UpstreamError` | 502 | `upstream_error` |
| `BadRequest` | 400 | `bad_request` |
| `Internal` | 500 | `internal_error` |

---

## 10. Billing e Preços

Cada modelo tem preço configurado em `pricing_per_model`:
- `input_price_per_1k`: custo por 1k tokens de entrada (em centavos de centavo / 1/10000 USD)
- `output_price_per_1k`: custo por 1k tokens de saída

Após cada request, `billing::charge_request` calcula o custo e acumula em `billing_records` do período (mês atual). Alertas automáticos são gerados quando o usuário atinge 80% ou 100% da quota mensal.

Planos de assinatura (Free / Pro / Enterprise) definem `monthly_limit_tokens` e são seedados pela migration 002.

## 11. Administração

O painel admin expõe endpoints protegidos por JWT + role `admin`:
- **Pricing**: CRUD de preços por modelo
- **Provider Health**: health check manual e histórico de latência/disponibilidade
- **Metrics**: total de usuários, usuários ativos (7d), requests, tokens, receita e top modelos

## 12. Deployment

### Docker Compose (desenvolvimento local)

```bash
cd provider-service
cp .env.example .env   # edite as variáveis
docker compose up -d
```

O `docker-compose.yml` sobe:
- PostgreSQL 16
- Redis 7
- hyscode-gateway (build local)

### Migrations

```bash
# Requer sqlx-cli: cargo install sqlx-cli
DATABASE_URL=postgres://... sqlx migrate run --source migrations/
```

A migration `001_initial.sql` cria as tabelas: `users`, `api_keys`, `requests_log`, `usage_quotas`.
A migration `002_billing_admin.sql` adiciona: `subscription_plans`, `subscriptions`, `pricing_per_model`, `billing_records`, `usage_alerts`, `provider_health`, além de seed dos planos padrão.

---

## 13. Fluxo Completo de uma Requisição

```
Cliente
  │  POST /v1/chat/completions
  │  Authorization: Bearer hsk_abc123
  ▼
auth_middleware
  │  1. Extrai "hsk_abc123"
  │  2. SHA-256(token) → hash
  │  3. SELECT em api_keys WHERE key_hash = hash
  │  4. Verifica is_active, expires_at
  │  5. Injeta AuthContext na request
  ▼
chat_completions_handler
  │  1. Lê AuthContext (user_id, scopes)
  │  2. Verifica scope "chat"
  │  3. Resolve provedor via AppState::resolve_provider(model)
  │  4. Encaminha request para upstream (Anthropic / OpenAI)
  │  5a. stream=false → aguarda resposta completa → JSON
  │  5b. stream=true  → SSE passthrough
  │  6. Persiste log em requests_log
  │  7. Atualiza usage_quotas
  ▼
Cliente recebe resposta OpenAI-compatible
```
