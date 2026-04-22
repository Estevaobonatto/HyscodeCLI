# HyscodeCLI - Agente de Codificação em CLI

> **Status:** Fase 2 e 3 implementadas — Core da CLI, Provedores, Engine e Ferramentas  
> **Versão:** 0.1.0-alpha  
> **Linguagem:** Rust  
> **Licença:** Proprietária (CLI) + SaaS (Provedor Próprio)

## Visão Geral

HyscodeCLI é uma ferramenta de linha de comando que funciona como um agente de codificação inteligente, conectando-se a múltiplos provedores de modelos de linguagem (LLMs) para auxiliar desenvolvedores em tarefas de software engineering diretamente no terminal.

A CLI suporta provedores externos (OpenAI, Anthropic, GitHub Copilot, Z.ai, OpenRouter) e um **provedor próprio SaaS** que abstrai múltiplos modelos sob uma única chave API paga, gerenciada pela equipe HyscodeCLI.

## Funcionalidades Implementadas


| Funcionalidade                          | Status | Crate                              |
| --------------------------------------- | ------ | ---------------------------------- |
| Chat interativo com streaming           | ✅      | `hyscode-cli`                      |
| Modo agente autônomo com ferramentas    | ✅      | `hyscode-cli` + `hyscode-engine`   |
| Gerenciamento de provedores             | ✅      | `hyscode-cli` + `hyscode-provider` |
| Adapters OpenAI, Anthropic, Hyscode     | ✅      | `hyscode-provider`                 |
| Tool calling (read/write/search/exec)   | ✅      | `hyscode-tools` + `hyscode-engine` |
| Permission Manager (audit/auto-approve) | ✅      | `hyscode-engine`                   |
| Sistema de Tasks com prioridade         | ✅      | `hyscode-engine`                   |
| Markdown + syntax highlighting          | ✅      | `hyscode-ui`                       |
| Geração de mensagens de commit          | ✅      | `hyscode-cli`                      |
| Persistência de conversas (SQLite)      | ✅      | `hyscode-engine`                   |
| TUI com ratatui                         | ✅      | `hyscode-ui`                       |


## Provedores Suportados


| Provedor             | Tipo               | Autenticação      | Status Adapter           |
| -------------------- | ------------------ | ----------------- | ------------------------ |
| Anthropic (Claude)   | Externo            | API Key           | ✅ Implementado           |
| OpenAI               | Externo            | API Key           | ✅ Implementado           |
| GitHub Copilot       | Externo            | OAuth / Token     | 🔄 Via OpenAI-compatible |
| Z.ai                 | Externo            | API Key           | 🔄 Via OpenAI-compatible |
| OpenRouter           | Externo            | API Key           | 🔄 Via OpenAI-compatible |
| **Hyscode Provider** | **Próprio (SaaS)** | **API Key única** | ✅ Implementado           |


## Estrutura de Documentação

```
docs/
├── architecture/          # Documentação arquitetural
│   ├── ARQUITETURA.md     # Visão geral da arquitetura
│   ├── MODELOS_C4.md      # Diagramas C4
│   └── FLUXOS.md          # Fluxos de execução
├── requirements/          # Requisitos
│   └── REQUISITOS.md      # Requisitos funcionais e não-funcionais
├── adr/                   # Architecture Decision Records
│   ├── ADR-001-provider-abstraction.md
│   ├── ADR-002-cli-architecture.md
│   ├── ADR-003-provider-service.md
│   ├── ADR-004-local-storage-rag.md
│   └── ADR-005-harness-task-system.md  # NOVO
├── api/                   # Contratos de API
│   └── provider-api.md    # API do provedor próprio
└── provider-service/      # Arquitetura do serviço SaaS
    └── ARQUITETURA_SERVICO.md
```

## Stack Tecnológico


| Camada           | Tecnologia                            |
| ---------------- | ------------------------------------- |
| CLI              | Rust, clap, tokio, ratatui            |
| HTTP Client      | reqwest, hyper                        |
| Streaming        | tokio-stream, async-stream            |
| Config           | serde, toml, directories              |
| Logs             | tracing, tracing-subscriber           |
| Testes           | tokio-test, wiremock, mockall         |
| Syntax Highlight | syntect, pulldown-cmark               |
| Banco Local      | SQLite (sqlx)                         |
| Provider Service | Rust (axum) / Go / Python (a definir) |
| Banco de Dados   | PostgreSQL + Redis                    |
| Filas            | Redis / RabbitMQ                      |
| Deploy           | Docker, Kubernetes                    |


## Quick Start

```bash
# Build do workspace
cargo build --workspace

# Configuração inicial
hyscode init

# Configurar provedor
hyscode provider add openai --api-key $OPENAI_API_KEY
hyscode provider add hyscode --api-key $HYSCODE_API_KEY

# Chat interativo
hyscode chat "Refatore este código para usar async/await"

# Modo agente autônomo
hyscode agent --task "Implemente uma API REST com autenticação JWT"

# Geração de commit
hyscode commit --all

# Executar testes
cargo test --workspace
```

## Arquitetura de Crates

```
hyscode/
├── crates/
│   ├── hyscode-cli/        # Binário principal (clap, comandos)
│   ├── hyscode-core/       # Domínio: types, traits, errors
│   ├── hyscode-engine/     # Orquestração: AgentLoop, ContextBuilder,
│   │                        #   ConversationManager, PermissionManager,
│   │                        #   TokenEstimator, TaskSystem
│   ├── hyscode-provider/   # Adapters de provedores (OpenAI, Anthropic,
│   │                        #   Hyscode SaaS) + HTTP client + SSE
│   ├── hyscode-tools/      # Ferramentas do agente: read_file, write_file,
│   │                        #   list_dir, search_code, execute_command, git_diff
│   ├── hyscode-ui/         # Interface terminal: TUI (ratatui),
│   │                        #   streaming, markdown renderer
│   ├── hyscode-config/     # Config TOML, keyring, env vars
│   └── hyscode-macros/     # Procedural macros internas
```

## Roadmap

- [x] Fase 1: Planejamento e Arquitetura
- [x] Fase 2: Core da CLI e abstração de provedores
- [x] Fase 3: Engine completo, AgentLoop, tools, streaming, Task System
- [ ] Fase 4: Desenvolvimento do Provider Service SaaS
- [ ] Fase 5: Sistema de pagamentos e billing (Stripe em dolar)
- [ ] Fase 6: Testes, documentação e release

## Comandos Disponíveis


| Comando                      | Descrição                       | Status |
| ---------------------------- | ------------------------------- | ------ |
| `hyscode chat [mensagem]`    | Chat interativo com TUI         | ✅      |
| `hyscode agent --task "..."` | Agente autônomo com ferramentas | ✅      |
| `hyscode commit [--all]`     | Gera mensagem de commit com LLM | ✅      |
| `hyscode provider &lt;add    | list                            | remove |
| `hyscode config &lt;get      | set                             | show   |
| `hyscode init`               | Inicializa configuração         | ✅      |


---

**HyscodeCLI** — Codifique mais, configure menos.