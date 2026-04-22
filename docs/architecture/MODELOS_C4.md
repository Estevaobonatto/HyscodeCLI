# Modelos C4 — HyscodeCLI

> **Metodologia:** C4 Model (Simon Brown)  
> **Foco:** Níveis 1 (Sistema), 2 (Container), 3 (Componente)

---

## Nível 1: Diagrama de Contexto do Sistema

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│   ┌──────────────┐                                                           │
│   │ Desenvolvedor│                                                           │
│   │   (Pessoa)   │                                                           │
│   └──────┬───────┘                                                           │
│          │ Usa para codificar, refatorar, revisar código                     │
│          ▼                                                                   │
│   ┌─────────────────────────────────────────┐                               │
│   │      HyscodeCLI — Sistema de Agentes    │                               │
│   │         de Codificação em CLI           │                               │
│   └──────────────┬──────────────────────────┘                               │
│                  │ Requisições HTTPS/SSE                                      │
│          ┌───────┴───────┬──────────────┬──────────────┐                     │
│          ▼               ▼              ▼              ▼                     │
│   ┌──────────┐    ┌──────────┐   ┌──────────┐   ┌────────────────┐         │
│   │ Anthropic│    │  OpenAI  │   │OpenRouter│   │Hyscode Provider│         │
│   │ (Claude) │    │ (GPT)    │   │ (Proxy)  │   │  Service (SaaS)│         │
│   └──────────┘    └──────────┘   └──────────┘   └────────────────┘         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Atores

| Ator | Descrição |
|------|-----------|
| **Desenvolvedor** | Usuário final que interage com a CLI para tarefas de codificação |

### Sistemas Externos

| Sistema | Tipo | Protocolo | Descrição |
|---------|------|-----------|-----------|
| Anthropic Claude | SaaS | HTTPS + SSE | API oficial do Claude |
| OpenAI | SaaS | HTTPS + SSE | API oficial da OpenAI |
| GitHub Copilot | SaaS | HTTPS + SSE | API do Copilot (via OAuth) |
| Z.ai | SaaS | HTTPS + SSE | API da Z.ai |
| OpenRouter | SaaS | HTTPS + SSE | Agregador de modelos |
| Hyscode Provider | SaaS (Próprio) | HTTPS + SSE | Proxy unificado multi-model |

---

## Nível 2: Diagrama de Containers

```text
┌────────────────────────────────────────────────────────────────────────────────┐
│                           Sistema: HyscodeCLI                                   │
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                              CLI Application                              │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │  │
│  │  │  Parser  │  │  Engine  │  │Provider  │  │  Tools   │  │   UI     │  │  │
│  │  │ (clap)   │  │(tokio)   │  │ Adapters │  │ (async)  │  │(ratatui) │  │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                           Dados Locais                                    │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌──────────────────┐  │  │
│  │  │ Config     │  │  Cache     │  │ Histórico  │  │  Índice RAG      │  │  │
│  │  │(TOML/YAML) │  │(SQLite)    │  │(SQLite)    │  │  (vector store)  │  │  │
│  │  └────────────┘  └────────────┘  └────────────┘  └──────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                         Sistema Operacional                               │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐                         │  │
│  │  │ Keyring    │  │ Filesystem │  │   Git      │                         │  │
│  │  │(secrets)   │  │(projeto)   │  │(repo local)│                         │  │
│  │  └────────────┘  └────────────┘  └────────────┘                         │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
└────────────────────────────────────────────────────────────────────────────────┘
```

### Containers da CLI

| Container | Tecnologia | Responsabilidade |
|-----------|-----------|------------------|
| **Parser** | `clap` | Parse de argumentos, validação, geração de help |
| **Engine** | `tokio` + Rust async | Orquestração de conversas, execução do loop do agente |
| **Provider Adapters** | `reqwest` + `serde` | Abstração sobre APIs de LLM |
| **Tools** | Rust std + `tokio::process` | Execução de ferramentas do agente |
| **UI** | `ratatui` / `console` | Renderização no terminal |

### Dados Locais

| Armazenamento | Formato | Conteúdo |
|--------------|---------|----------|
| Configuração | TOML | Provedores, preferências, aliases |
| Cache | SQLite | Respostas cacheadas, embeddings |
| Histórico | SQLite | Conversas, threads |
| Índice RAG | SQLite + `rusqlite` ou `pgvector` local | Vetores de código para busca semântica |

---

## Nível 3: Diagrama de Componentes (Engine)

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                            Container: Engine                                  │
│                                                                               │
│   ┌─────────────────────────────────────────────────────────────────────┐    │
│   │                        Conversation Manager                          │    │
│   │   - Mantém estado da conversa (mensagens, tokens)                   │    │
│   │   - Persistência em SQLite                                          │    │
│   │   - Suporte a múltiplas threads                                     │    │
│   └────────┬────────────────────────────────────────────────────┬─────────┘    │
│            │                                                    │              │
│            ▼                                                    ▼              │
│   ┌─────────────────┐                                ┌─────────────────┐      │
│   │  Context Builder │                                │  Token Manager  │      │
│   │                  │                                │                 │      │
│   │  - File resolver │                                │  - Contagem     │      │
│   │  - Git context   │                                │  - Limites      │      │
│   │  - System prompt │                                │  - Estimativa   │      │
│   │  - RAG retrieval │                                │    de custo     │      │
│   └────────┬─────────┘                                └─────────────────┘      │
│            │                                                                   │
│            ▼                                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐     │
│   │                         Tool Dispatcher                            │     │
│   │   - Registro de ferramentas disponíveis                            │     │
│   │   - Validação de permissões (confirmar escrita/execução)           │     │
│   │   - Execução e retorno de resultados                               │     │
│   │   - Suporte a auto-approve e audit-only                            │     │
│   └────────┬────────────────────────────────────────────────────┬─────────┘     │
│            │                                                    │               │
│            ▼                                                    ▼               │
│   ┌─────────────────┐                                ┌─────────────────┐       │
│   │  Provider Port   │◄──────────────────────────────►│ Provider Adapter│       │
│   │  (trait)         │     Requisição/Resposta        │ (impl concrete) │       │
│   │                  │                                │                 │       │
│   │  - chat()        │                                │ - OpenAI        │       │
│   │  - stream()      │                                │ - Anthropic     │       │
│   │  - list_models() │                                │ - Copilot       │       │
│   │  - validate()    │                                │ - OpenRouter    │       │
│   └─────────────────┘                                │ - Hyscode       │       │
│                                                      └─────────────────┘       │
└────────────────────────────────────────────────────────────────────────────────┘
```

### Componentes da Engine

| Componente | Interface | Descrição |
|-----------|-----------|-----------|
| **Conversation Manager** | `ConversationService` | Estado e persistência de conversas |
| **Context Builder** | `ContextBuilder` | Monta o contexto completo do prompt |
| **Token Manager** | `TokenEstimator` | Contagem e limitação de tokens |
| **Tool Dispatcher** | `ToolRegistry` + `ToolExecutor` | Gerenciamento de ferramentas |
| **Provider Port** | `trait Provider` | Porta da arquitetura hexagonal |

---

## Nível 3: Diagrama de Componentes (Provider Service SaaS)

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         Sistema: Hyscode Provider Service                        │
│                                                                                  │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │                           API Gateway (axum/rocket)                        │  │
│  │   - Rate Limiting (Redis)                                                 │  │
│  │   - Autenticação (API Key)                                                │  │
│  │   - Logging / Tracing                                                     │  │
│  └─────────────────────────┬─────────────────────────────────────────────────┘  │
│                            │                                                    │
│                            ▼                                                    │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │                         Request Router                                     │  │
│  │   - Parse do body (OpenAI-compatible)                                     │  │
│  │   - Seleção de modelo/provedor upstream                                   │  │
│  │   - Fallback logic                                                        │  │
│  └─────────────────────────┬─────────────────────────────────────────────────┘  │
│                            │                                                    │
│           ┌────────────────┼────────────────┐                                   │
│           ▼                ▼                ▼                                   │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                           │
│  │  OpenAI     │   │  Anthropic  │   │  OpenRouter │   ... outros provedores   │
│  │  Adapter    │   │  Adapter    │   │  Adapter    │                           │
│  └─────────────┘   └─────────────┘   └─────────────┘                           │
│                                                                                  │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │                         Billing & Usage                                    │  │
│  │   - Contabilização de tokens/requisições                                  │  │
│  │   - Planos e limites                                                      │  │
│  │   - Cobrança (Stripe)                                                     │  │
│  └───────────────────────────────────────────────────────────────────────────┐  │
│                                                                                  │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │                         Data Store                                         │  │
│  │   PostgreSQL (users, api_keys, usage)  │  Redis (cache, rate limits)      │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Relacionamentos Principais

| De | Para | Relação | Tecnologia |
|----|------|---------|-----------|
| Desenvolvedor | CLI Application | Usa | Terminal |
| CLI Application | Provider Adapters | Faz requisições | HTTPS + SSE |
| CLI Application | Keyring | Armazena secrets | OS API |
| CLI Application | Filesystem | Lê/Escreve arquivos | std::fs |
| CLI Application | Git | Obtém contexto | `git` CLI ou `git2` |
| Provider Service | Provedores Upstream | Encaminha requisições | HTTPS + SSE |
| Provider Service | PostgreSQL | Persistência | `sqlx` / `diesel` |
| Provider Service | Redis | Cache e rate limits | `redis` |
