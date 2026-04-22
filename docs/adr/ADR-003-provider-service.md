# ADR-003: Arquitetura do Hyscode Provider Service (SaaS)

> **Status:** Aceito  
> **Data:** 2026-04-22  
> **Decisores:** Arquiteto, CTO, Product Owner

## Contexto

O Hyscode Provider Service é um serviço SaaS proprietário que oferece:

1. **Uma única chave API** para acessar múltiplos modelos de diferentes provedores
2. **Roteamento inteligente** entre provedores upstream
3. **Gestão de custos** unificada para usuários
4. **Modelos próprios** (futuramente fine-tuned ou via partnerships)

Este serviço é o diferencial comercial do HyscodeCLI e a principal fonte de receita (modelo freemium + assinaturas).

## Decisão

O serviço será construído como uma **API Gateway + Router** que expõe uma interface **OpenAI-compatible** para os clientes, e se comunica com provedores upstream via adapters próprios.

### Por que OpenAI-compatible?

- Ecossistema maduro e familiar para desenvolvedores
- Facilita migração de usuários existentes
- Nosso adapter de Hyscode na CLI pode reutilizar lógica do OpenAIAdapter
- Bibliotecas de terceiros já suportam este formato

### Arquitetura de Alto Nível

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           Hyscode Provider Service                               │
│                                                                                  │
│   ┌───────────────────────────────────────────────────────────────────────────┐ │
│   │  Layer 1: Edge / Load Balancer (Cloudflare / AWS ALB / Nginx)            │ │
│   │  - TLS termination                                                        │ │
│   │  - DDoS protection                                                        │ │
│   │  - Geo-routing                                                            │ │
│   └───────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                             │
│   ┌────────────────────────────────▼──────────────────────────────────────────┐ │
│   │  Layer 2: API Gateway (axum / Actix-web / Go)                             │ │
│   │                                                                           │ │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │ │
│   │  │  Auth       │  │ Rate Limit  │  │  Logging    │  │  Request Parser │  │ │
│   │  │  Middleware │  │  Middleware │  │  Middleware │  │  (OpenAI fmt)   │  │ │
│   │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────┘  │ │
│   └───────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                             │
│   ┌────────────────────────────────▼──────────────────────────────────────────┐ │
│   │  Layer 3: Router & Orchestrator (Rust/Go)                                 │ │
│   │                                                                           │ │
│   │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐   │ │
│   │  │  Model Resolver │  │  Load Balancer  │  │  Fallback Manager       │   │ │
│   │  │  (alias → real) │  │  (least-cost/   │  │  (retry next provider)  │   │ │
│   │  │                 │  │   round-robin)  │  │                         │   │ │
│   │  └─────────────────┘  └─────────────────┘  └─────────────────────────┘   │ │
│   └───────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                             │
│   ┌────────────────────────────────▼──────────────────────────────────────────┐ │
│   │  Layer 4: Provider Adapters (Rust/Go/Python)                              │ │
│   │                                                                           │ │
│   │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │ │
│   │  │ OpenAI   │ │Anthropic │ │OpenRouter│ │  Z.ai    │ │  Custom  │        │ │
│   │  │ Adapter  │ │ Adapter  │ │ Adapter  │ │ Adapter  │ │  Models  │        │ │
│   │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘        │ │
│   └───────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                             │
│   ┌────────────────────────────────▼──────────────────────────────────────────┐ │
│   │  Layer 5: Data & Cache                                                    │ │
│   │                                                                           │ │
│   │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐   │ │
│   │  │  PostgreSQL     │  │  Redis          │  │  Object Storage         │   │ │
│   │  │  (users, usage, │  │  (rate limits,  │  │  (logs, backups)        │   │ │
│   │  │   billing)      │  │   cache)        │  │                         │   │ │
│   │  └─────────────────┘  └─────────────────┘  └─────────────────────────┘   │ │
│   └───────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Stack Tecnológico do Serviço

| Componente | Tecnologia | Justificativa |
|-----------|-----------|---------------|
| API Gateway | **Rust (axum)** ou **Go (gin/echo)** | Performance, baixa latência, async nativo |
| Router | Mesmo da gateway | Acesso direto ao estado |
| Adapters | Rust ou Go | Reutilização de lógica da CLI |
| Database | **PostgreSQL 15+** | Dados relacionais complexos, ACID |
| Cache | **Redis** | Rate limits, cache de respostas, sessions |
| Message Queue | **Redis Streams** ou **NATS** | Filas de billing assíncrono |
| Observability | **Prometheus** + **Grafana** + **Jaeger** | Métricas, dashboards, tracing |
| Billing | **Stripe** | Gateway de pagamento |
| Deploy | **Docker** + **Kubernetes** (EKS/GKE) | Orquestração, auto-scaling |

### Especificação da API

**Base URL:** `https://api.hyscode.dev/v1`

**Autenticação:**
```http
Authorization: Bearer hsk_<key>
```

**Endpoints principais:**

```
POST   /v1/chat/completions
GET    /v1/models
POST   /v1/embeddings         (futuro)
GET    /v1/usage              (dashboard)
POST   /v1/api-keys           (gerenciamento)
```

**Compatibilidade OpenAI:**
- Request/response seguem exatamente o schema da OpenAI
- Campos extras são ignorados (forward compatible)
- Extensions via `x-hyscode-*` headers (non-breaking)

### Modelos e Alias

```json
{
  "object": "list",
  "data": [
    {
      "id": "hyscode-fast",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "hyscode": {
        "provider": "anthropic",
        "model": "claude-3-haiku-20240307",
        "description": "Rápido e econômico para tarefas simples"
      }
    },
    {
      "id": "hyscode-smart",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "hyscode": {
        "provider": "anthropic",
        "model": "claude-3-5-sonnet-20241022",
        "description": "Melhor custo-benefício para coding"
      }
    },
    {
      "id": "hyscode-ultra",
      "object": "model",
      "created": 1713792000,
      "owned_by": "hyscode",
      "hyscode": {
        "provider": "openai",
        "model": "gpt-4o",
        "description": "Máxima capacidade de raciocínio"
      }
    }
  ]
}
```

### Roteamento Inteligente

O Router considera:

1. **Modelo solicitado:** Resolve alias para provedor+modelo real
2. **Disponibilidade:** Health check contínuo dos provedores upstream
3. **Custo:** Seleção baseada em preço por token (configurável por plano)
4. **Performance:** Latência recente do provedor
5. **Fallback:** Se provedor A falhar, tenta B automaticamente (se compatível)

### Rate Limiting

```
┌────────────────────────────────────────┐
│  Estratégia: Token Bucket (Redis)      │
│                                        │
│  Níveis:                               │
│  - Por API Key (usuário)               │
│  - Por Modelo                          │
│  - Por Provedor Upstream (proteção)    │
│                                        │
│  Headers de resposta:                  │
│  X-RateLimit-Limit: 100                │
│  X-RateLimit-Remaining: 42             │
│  X-RateLimit-Reset: 1713795600         │
└────────────────────────────────────────┘
```

### Billing e Planos

| Plano | Preço | Limites | Features |
|-------|-------|---------|----------|
| **Free** | Grátis | 50 req/dia, modelos básicos | Comunidade |
| **Pro** | $19/mês | Ilimitado*, modelos premium | Prioridade, support |
| **Team** | $49/user/mês | Ilimitado*, SSO, audit logs | Admin, analytics |
| **Enterprise** | Custom | Custom | SLA, dedicated, fine-tuning |

\* Sujeito a fair use policy

**Mecanismo de cobrança:**
1. Cada request é contabilizada (tokens input + output)
2. Preço por token varia por modelo subjacente + markup Hyscode
3. Uso acumulado em tempo real
4. Alertas em 80% do limite
5. Throttling ou overage billing ao exceder

### Segurança

- **API Keys:** Prefixo `hsk_`, 48 chars, geradas via CSPRNG
- **Rotação:** Suportada via API, revogação instantânea
- **TLS:** Obrigatório, TLS 1.3
- **Audit logs:** Todas as requisições logadas (sem conteúdo de prompt, a menos que opt-in)
- **GDPR/LGPD:** Deleção de dados sob demanda, DPO designado

## Consequências

### Positivas
- **Simplicidade para usuários:** Uma chave, múltiplos modelos
- **Controle comercial:** Podemos ajustar margens, modelos disponíveis, features por plano
- **Agregação:** Analytics unificados de uso
- **Caching:** Possibilidade de cachear respostas comuns (compliance permitindo)

### Negativas
- **Ponto único de falha:** Se nosso serviço cair, usuários perdem acesso
- **Latência adicional:** Hop extra entre cliente e provedor final
- **Complexidade operacional:** Precisamos monitorar múltiplos provedores upstream
- **Custo de infraestrutura:** Servidores, banda, storage

## Alternativas Consideradas

| Alternativa | Descrição | Motivo do descarte |
|-------------|-----------|-------------------|
| Apenas proxy transparente | Forward direto sem roteamento inteligente | Não agrega valor; não permite billing unificado |
| GraphQL API | Schema flexível | Complexo; ecossistema OpenAI já usa REST |
| gRPC internamente | Performance | Boa ideia para interno; manter REST externo |
| Serverless (Lambda/Functions) | Menos operação | Cold start inaceitável para streaming; custo imprevisível |

## Roadmap do Serviço

| Fase | Feature | Prazo |
|------|---------|-------|
| 1 | API Gateway + Auth + OpenAI/Anthropic adapters | Mês 1-2 |
| 2 | Roteamento inteligente + Fallback | Mês 2-3 |
| 3 | Billing + Stripe + Planos | Mês 3-4 |
| 4 | Dashboard + Analytics + Admin | Mês 4-5 |
| 5 | Cache + Otimizações de custo | Mês 5-6 |
| 6 | Modelos próprios / Fine-tuning | Mês 6+ |
