# Arquitetura do HyscodeCLI

> **Nível:** Visão Geral (Nível 1 C4)  
> **Público-alvo:** Arquitetos, Tech Leads, Desenvolvedores  
> **Última atualização:** 2026-04-22

## 1. Visão Arquitetural

O HyscodeCLI é composto por duas macro-partes:

1. **CLI (Cliente):** Aplicação Rust que roda na máquina do desenvolvedor
2. **Provider Service (SaaS):** Serviço gerenciado que atua como proxy unificado para múltiplos provedores de LLM

```
┌─────────────────────────────────────────────────────────────────────┐
│                         MÁQUINA DO DESENVOLVEDOR                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     HyscodeCLI (Rust)                         │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │  │
│  │  │  CLI App │  │  Engine  │  │ Provider │  │  Tool Runner │ │  │
│  │  │ (clap)   │  │ (Agent)  │  │ Adapter  │  │ (Sandbox)    │ │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────────┘ │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                    │  │
│  │  │  Tasks   │  │ Permission│  │   UI     │                    │  │
│  │  │ System   │  │ Manager  │  │ (TUI)    │                    │  │
│  │  └──────────┘  └──────────┘  └──────────┘                    │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                           │                                         │
│                    HTTPS / SSE / WebSocket                          │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
┌───────────────────────────┼─────────────────────────────────────────┐
│                    INTERNET │                                        │
│  ┌────────┐  ┌────────┐  ┌┴───────┐  ┌────────┐  ┌────────────────┐│
│  │OpenAI  │  │Anthropic│  │OpenRouter│  │ Z.ai   │  │ Hyscode        ││
│  │        │  │         │  │          │  │        │  │ Provider       ││
│  └────────┘  └────────┘  └──────────┘  └────────┘  │ Service (SaaS) ││
│                                                     │                ││
│  ┌────────┐  ┌────────┐                             │ ┌────────────┐ ││
│  │GitHub  │  │Google  │                             │ │ API Gateway│ ││
│  │Copilot │  │Gemini  │                             │ │ Router     │ ││
│  └────────┘  └────────┘                             │ │ Billing    │ ││
│                                                     │ └────────────┘ ││
│                                                     └────────────────┘│
└───────────────────────────────────────────────────────────────────────┘
```

## 2. Componentes da CLI

### 2.1. CLI App (`hyscode-cli`)

- **Responsabilidade:** Parse de argumentos, comandos, subcomandos
- **Tecnologia:** `clap` (derive macros)
- **Comandos implementados:**
  - `hyscode chat [mensagem]` — Modo conversação com TUI e streaming
  - `hyscode agent --task <descricao>` — Modo agente autônomo com ferramentas
  - `hyscode commit [--all]` — Gera mensagem de commit com LLM
  - `hyscode review [--staged]` — Revisa diff atual com o LLM
  - `hyscode provider <add|list|remove|default|test|login|models>` — Gestão de provedores
  - `hyscode config <get|set|show|edit>` — Configurações
  - `hyscode init` — Inicialização do projeto
  - `hyscode history [-n <limite>]` — Lista histórico de conversas
  - `hyscode undo [passos]` — Desfaz escritas de arquivo pelo agente
  - `hyscode completions <shell>` — Gera script de autocompleção (bash, zsh, fish, powershell, elvish)

### 2.2. Engine (`hyscode-engine`)

- **Responsabilidade:** Orquestração de conversas, gerenciamento de estado, execução de ferramentas, sistema de tasks
- **Componentes internos:**
  - **AgentLoop:** Harness central que executa tarefas autônomas com loop de iterações, retry e eventos
  - **ConversationManager:** Persistência de histórico em SQLite (sqlx)
  - **ContextBuilder:** Monta o contexto a partir de arquivos, git, histórico
  - **PermissionManager:** Policy engine para controle de acesso às ferramentas (audit-only, auto-approve, callbacks)
  - **TokenEstimator:** Calcula e limita tokens de acordo com o modelo
  - **Summarizer:** Sumariza mensagens antigas quando contexto ultrapassa 85% do limite
  - **AuditLog:** Persiste cada execução de ferramenta em JSONL para rastreabilidade
  - **TaskSystem:** Orquestração de múltiplas tarefas com fila prioritária, retries e eventos

### 2.3. Provider Adapter (`hyscode-provider`)

- **Responsabilidade:** Abstração sobre múltiplos provedores de LLM
- **Padrão:** Adapter + Strategy
- **Implementações:**
  - `OpenAIAdapter` — ✅ Completo (chat, streaming, list_models, validate)
  - `AnthropicAdapter` — ✅ Completo (Messages API + SSE)
  - `HyscodeProviderAdapter` — ✅ Completo (OpenAI-compatible, base_url custom)
  - `GitHubCopilotAdapter` — ✅ Completo (delega ao OpenAI adapter; requer token OAuth)
  - `OpenRouterAdapter` — ✅ Completo (delega ao OpenAI adapter; default model `openai/gpt-4o`)
  - `ZAiAdapter` — ✅ Completo (delega ao OpenAI adapter; base_url `https://api.z.ai/v1`; default model `z-pro`)
  - `GeminiAdapter` — ✅ Completo (Google AI Studio REST API + SSE; modelos Gemini 2.x/3.x)

### 2.4. Tool Runner (`hyscode-tools`)

- **Responsabilidade:** Execução de ferramentas disponibilizadas ao modelo
- **Ferramentas implementadas:**
  - `read_file` — Leitura de arquivo (não-destrutiva, auto-approve)
  - `write_file` — Escrita de arquivo (destrutiva, requer confirmação)
  - `list_dir` — Listagem de diretório (não-destrutiva)
  - `search_code` — Busca recursiva em código com filtro de glob e limite de resultados
  - `glob_search` — Busca de arquivos por padrão glob
  - `execute_command` — Execução de comando shell com timeout (destrutiva)
  - `git_diff` — Obtém diff do repositório

### 2.5. Interface (`hyscode-ui`)

- **Responsabilidade:** Renderização no terminal
- **Modos implementados:**
  - **Stream:** Saída contínua de texto com eventos
  - **TUI:** Interface interativa com `ratatui`
  - **Markdown:** Renderização com `pulldown-cmark` + syntax highlighting com `syntect`

### 2.6. Configuração (`hyscode-config`)

- **Responsabilidade:** Gerenciamento de configurações e secrets
- **Armazenamento:**
  - Configurações: `~/.config/hyscode/config.toml`
  - Secrets: Keyring do SO (via `keyring` crate ou `secret-service`)
  - Cache: `~/.cache/hyscode/`
  - Dados: `~/.local/share/hyscode/` (SQLite de conversas)

## 3. Camadas da Arquitetura

```
┌─────────────────────────────────────────┐
│  Camada de Apresentação (UI/CLI)        │
│  clap, ratatui, console, dialoguer      │
├─────────────────────────────────────────┤
│  Camada de Aplicação (Engine)           │
│  AgentLoop, TaskSystem, Conversations,  │
│  Context, Tools, PermissionManager      │
├─────────────────────────────────────────┤
│  Camada de Domínio (Core)               │
│  Models, Providers, Messages, Tokens    │
├─────────────────────────────────────────┤
│  Camada de Infraestrutura               │
│  HTTP Client, Keyring, Filesystem, Git, │
│  SQLite, SSE Parser                     │
└─────────────────────────────────────────┘
```

## 4. Fluxo de Dados

### 4.1. Fluxo de Requisição (Chat)

```
Usuário → CLI Parser → Engine → Context Builder → Provider Adapter
                                                          ↓
Usuário ← UI Renderer ← Engine ← Token Estimator ← HTTP Client ← Provedor
```

### 4.2. Fluxo de Execução de Ferramenta (Agent)

```
Engine → Requisição ao Provedor → Resposta com tool_calls
  ↓
PermissionManager → Validação de Permissões → Execução
  ↓
Engine → Requisição de Follow-up (com resultado) → Resposta final
```

### 4.3. Fluxo do Sistema de Tasks

```
Usuário → Submit Task → TaskQueue (priorizada)
  ↓
TaskRunner → AgentLoop → Provider + Tools
  ↓
TaskStore (persistência) + EventBus (streaming de progresso)
```

## 5. Padrões Arquiteturais

| Padrão           | Aplicação                                 |
| ---------------- | ----------------------------------------- |
| **Adapter**      | Unificar APIs de provedores diferentes    |
| **Strategy**     | Seleção de modelo/provedor em runtime     |
| **Command**      | Representação de operações do agente      |
| **Pipeline**     | Processamento de contexto e respostas     |
| **Event-driven** | Streaming de respostas via async channels |
| **Harness**      | AgentLoop orquestra Provider + Tools      |
| **Fail-closed**  | PermissionManager nega por padrão         |

## 6. Decisões Arquiteturais Principais

1. **Async-first:** Todo I/O é async via `tokio`
2. **Zero-copy onde possível:** Uso de `&str`, `Bytes`, `Arc<str>`
3. **Erros como valores:** `thiserror` para domínio, `anyhow` para aplicação
4. **Imutabilidade preferida:** Estruturas de dados imutáveis, clone-on-write
5. **Feature flags:** Cada provedor é uma feature Cargo opcional
6. **Harness pattern:** AgentLoop é o orquestrador central, desacoplado de providers e tools
7. **Event system:** `AgentEvent` e `TaskSystemEvent` para observabilidade e UI

## 7. Estrutura de Crates (Workspace)

```
hyscode/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── hyscode-cli/        # Binário principal
│   ├── hyscode-core/       # Domínio e modelos
│   ├── hyscode-engine/     # Orquestração (AgentLoop, Tasks, Permissions, Conversations)
│   ├── hyscode-provider/   # Abstração de provedores + adapters
│   ├── hyscode-tools/      # Ferramentas do agente
│   ├── hyscode-ui/         # Interface do terminal (TUI, Markdown, Streaming)
│   ├── hyscode-config/     # Configurações e secrets
│   └── hyscode-macros/     # Procedural macros internas
└── docs/
```

## 8. Segurança na Arquitetura

- **Secrets:** Nunca em disco em plaintext; sempre via keyring
- **Sandbox:** Execução de comandos com timeout (padrão 30s, máx 300s)
- **Rede:** Validação de certificados TLS, https_only
- **Input:** Sanitização de paths (prevenir path traversal), validação de comandos
- **Permissões:** PermissionManager é fail-closed — sem permissão explícita = negação
- **Audit-only:** Modo que loga mas nunca executa operações destrutivas

## 9. Métricas e Observabilidade

- **Tracing:** Todas as operações instrumentadas com `tracing`
- **Eventos:** `AgentEvent` e `TaskSystemEvent` via `mpsc::unbounded_channel`
- **Métricas:** Tempo de resposta por provedor, tokens por requisição, erros
- **Debug:** Exportação de conversas completas para arquivo (opt-in)
