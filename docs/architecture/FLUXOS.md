# Fluxos de Execução — HyscodeCLI

> **Propósito:** Detalhar os fluxos de dados e controle principais da aplicação  
> **Última atualização:** 2026-04-22

---

## Fluxo 1: Inicialização da CLI (`hyscode init`)

```
┌────────┐     ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ Usuário │────►│ CLI Parser  │────►│ Config Manager│────►│ Verifica    │
└────────┘     └─────────────┘     └──────────────┘     │ diretório   │
                                                        │ existente   │
                                                        └──────┬──────┘
                                                               │
                                          ┌────────────────────┼────────────────────┐
                                          ▼                    ▼                    ▼
                                    ┌──────────┐        ┌──────────┐          ┌──────────┐
                                    │  Existe  │        │  Cria    │          │   OK     │
                                    │ config?  │        │  novo    │          │          │
                                    └────┬─────┘        └──────────┘          └────┬─────┘
                                         │                                         │
                                    Sim ▼                                         │
                                    ┌──────────┐                                  │
                                    │ Pergunta │                                  │
                                    │ sobrescr?│                                  │
                                    └────┬─────┘                                  │
                                         │                                        │
                         ┌───────────────┼───────────────┐                       │
                         ▼               ▼               ▼                       │
                    ┌────────┐    ┌──────────┐     ┌────────┐                    │
                    │  Sim   │    │   Não    │     │ Merge  │                    │
                    └────┬───┘    └────┬─────┘     └───┬────┘                    │
                         │             │               │                         │
                         └─────────────┴───────────────┘                         │
                                          │                                      │
                                          ▼                                      ▼
                                    ┌──────────────────────────────────────────────────┐
                                    │ Cria/realiza configuração em:                    │
                                    │ ~/.config/hyscode/config.toml                    │
                                    │                                                  │
                                    │ Passos:                                          │
                                    │ 1. Perguntar modo de instalação                  │
                                    │ 2. Configurar provedor padrão                    │
                                    │ 3. Solicitar API key (armazenar no keyring)      │
                                    │ 4. Configurar preferências de UI                 │
                                    └──────────────────────────────────────────────────┘
```

---

## Fluxo 2: Comando de Chat (`hyscode chat "mensagem"`)

```
┌────────┐     ┌─────────────┐     ┌──────────────┐     ┌──────────────────┐
│ Usuário │────►│ CLI Parser  │────►│ Engine::chat │────►│ Context Builder   │
└────────┘     │ (valida args)│     └──────────────┘     └──────────────────┘
               └─────────────┘              │                    │
                                            │                    ▼
                                            │            ┌──────────────┐
                                            │            │ 1. Carrega   │
                                            │            │    system    │
                                            │            │    prompt    │
                                            │            │ 2. Resolve   │
                                            │            │    context   │
                                            │            │    files     │
                                            │            │ 3. Adiciona  │
                                            │            │    git info  │
                                            │            └──────┬───────┘
                                            │                   │
                                            ▼                   ▼
                                   ┌────────────────────────────────────┐
                                   │        Provider Adapter            │
                                   │  - Seleciona provedor ativo        │
                                   │  - Estima tokens                   │
                                   │  - Constrói request formatado      │
                                   │    (OpenAI, Anthropic, etc)        │
                                   └──────────────┬─────────────────────┘
                                                  │
                                                  ▼
                                   ┌────────────────────────────────────┐
                                   │         HTTP Transport             │
                                   │  - reqwest com timeout             │
                                   │  - retry com backoff               │
                                   │  - streaming SSE                   │
                                   └──────────────┬─────────────────────┘
                                                  │
                                                  ▼
                                   ┌────────────────────────────────────┐
                                   │        Provedor (remoto)           │
                                   │  - Processa prompt                 │
                                   │  - Retorna stream de tokens        │
                                   └──────────────┬─────────────────────┘
                                                  │
                                                  ▼
                                   ┌────────────────────────────────────┐
                                   │          UI Renderer               │
                                   │  - Recebe chunks via channel       │
                                   │  - Renderiza markdown              │
                                   │  - Syntax highlighting             │
                                   │  - Atualiza contador de tokens     │
                                   └──────────────┬─────────────────────┘
                                                  │
                                                  ▼
                                   ┌────────────────────────────────────┐
                                   │          Persistência              │
                                   │  - Salva mensagem no SQLite        │
                                   │  - Atualiza cache se necessário    │
                                   └────────────────────────────────────┘
```

---

## Fluxo 3: Modo Agente com Ferramentas (`hyscode agent --task ...`)

```
┌────────┐     ┌─────────────┐     ┌──────────────────────────────────────────┐
│ Usuário │────►│ CLI Parser  │────►│ AgentLoop::run                           │
└────────┘     └─────────────┘     │ 1. Recebe descrição da tarefa            │
                                   │ 2. Inicializa ambiente de trabalho       │
                                   └──────────────────────────────────────────┘
                                                    │
                                                    ▼
                                   ┌──────────────────────────────────────────┐
                                   │ LOOP DO AGENTE (max_iterations = 15)     │
                                   │                                          │
                                   │ ┌──────────────────────────────────────┐ │
                                   │ │ 1. Monta prompt com:                 │ │
                                   │ │    - system prompt (modo agente)     │ │
                                   │ │    - descrição da tarefa             │ │
                                   │ │    - resultados de tools anteriores  │ │
                                   │ │    - filesystem context              │ │
                                   │ └─────────────────┬────────────────────┘ │
                                   │                   │                      │
                                   │                   ▼                      │
                                   │ ┌──────────────────────────────────────┐ │
                                   │ │ 2. Envia ao provedor                 │ │
                                   │ │    (com tools disponíveis no schema) │ │
                                   │ └─────────────────┬────────────────────┘ │
                                   │                   │                      │
                                   │                   ▼                      │
                                   │ ┌──────────────────────────────────────┐ │
                                   │ │ 3. Recebe resposta do modelo         │ │
                                   │ │    ├── Texto → exibe e finaliza      │ │
                                   │ │    └── tool_calls → continua loop    │ │
                                   │ └─────────────────┬────────────────────┘ │
                                   │                   │                      │
                                   │          tool_calls?                     │
                                   │           /        \                     │
                                   │         Sim         Não                  │
                                   │          │          │                    │
                                   │          ▼          ▼                    │
                                   │ ┌──────────────┐  ┌──────────────┐      │
                                   │ │ 4. Permission│  │ 5. Finaliza  │      │
                                   │ │   Manager    │  │    tarefa    │      │
                                   │ └──────┬───────┘  └──────────────┘      │
                                   │        │                                │
                                   │        ▼                                │
                                   │ ┌──────────────────────────────────────┐ │
                                   │ │ 5. Verifica políticas:               │ │
                                   │ │    ├── audit-only? → só loga         │ │
                                   │ │    ├── auto-approve? → executa       │ │
                                   │ │    └── senão → callback interativo   │ │
                                   │ └──────────────┬───────────────────────┘ │
                                   │                │                         │
                                   │                ▼                         │
                                   │ ┌──────────────────────────────────────┐ │
                                   │ │ 6. Executa ferramenta                │ │
                                   │ │    - read_file: tokio::fs::read      │ │
                                   │ │    - write_file: tokio::fs::write    │ │
                                   │ │    - execute_cmd: tokio::process     │ │
                                   │ │    - search_code: walkdir+regex      │ │
                                   │ └──────────────┬───────────────────────┘ │
                                   │                │                         │
                                   │                ▼                         │
                                   │ ┌──────────────────────────────────────┐ │
                                   │ │ 7. Converte resultado em Message::Tool│ │
                                   │ │    e volta ao passo 1                │ │
                                   │ └──────────────────────────────────────┘ │
                                   └──────────────────────────────────────────┘
```

**Eventos emitidos durante o loop:**
- `AgentEvent::IterationStarted` — nova iteração
- `AgentEvent::ProviderCalled` — requisição ao provedor
- `AgentEvent::ToolExecuting` — tool sendo executada
- `AgentEvent::PermissionRequested` — aguardando confirmação
- `AgentEvent::ToolExecuted` — tool completou
- `AgentEvent::LoopFinished` — loop terminou

---

## Fluxo 4: Seleção de Provedor e Modelo

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Seleção de Provedor                               │
│                                                                              │
│   Entrada: --provider <nome>  ou  configuração padrão                       │
│                                                                              │
│   ┌─────────────┐                                                            │
│   │ --provider  │                                                            │
│   │ especificado?│                                                           │
│   └──────┬──────┘                                                            │
│          │                                                                   │
│    Sim ▼ │ Não                                                               │
│   ┌──────┴──────┐                                                            │
│   │ Usa o       │                                                            │
│   │ especificado│                                                            │
│   └──────┬──────┘                                                            │
│          │                                                                   │
│          ▼                                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                         Validação                                  │   │
│   │   1. Provedor está configurado? (tem API key?)                     │   │
│   │   2. Provedor está disponível? (health check)                      │   │
│   │   3. Modelo especificado existe na lista?                          │   │
│   │                                                                    │   │
│   │   Erro em qualquer passo → fallback para próximo na lista ou erro │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│          │                                                                   │
│          ▼                                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    Resolução de Modelo                             │   │
│   │                                                                    │   │
│   │   --model <nome>  →  usa direto                                   │   │
│   │   --model <alias> →  resolve: fast=..., smart=...                 │   │
│   │   (nada)          →  usa default do provedor                       │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│          │                                                                   │
│          ▼                                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                 Instanciação do Adapter                            │   │
│   │                                                                    │   │
│   │   match provider_name {                                            │   │
│   │       "openai"    => OpenAIAdapter::new(config),                  │   │
│   │       "anthropic" => AnthropicAdapter::new(config),               │   │
│   │       "copilot"   => OpenAIAdapter::new(config),                  │   │
│   │       "openrouter"=> OpenAIAdapter::new(config),                  │   │
│   │       "hyscode"   => HyscodeProviderAdapter::new(config),         │   │
│   │       _ => Err(ProviderNotFound)                                   │   │
│   │   }                                                                │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Fluxo 5: Sistema de Tasks (`TaskRunner`)

```
┌──────────┐      ┌───────────┐      ┌─────────────────────────────────────┐
│  Usuário  │─────►│ Submit    │─────►│ TaskQueue (fila com prioridade)    │
│  /CLI     │      │ Task      │      │  - Critical > High > Normal > Low  │
└──────────┘      └───────────┘      └──────────────────┬──────────────────┘
                                                       │
                                                       ▼
                                    ┌──────────────────────────────────────┐
                                    │ TaskRunner (loop em background)      │
                                    │  1. Dequeue task                     │
                                    │  2. Atualiza status → Running        │
                                    │  3. Executa via AgentLoop            │
                                    │  4. Se falha e can_retry → re-enfileira│
                                    │  5. Atualiza status → Completed/Failed│
                                    │  6. Emite TaskSystemEvent            │
                                    └──────────────────┬───────────────────┘
                                                       │
                                                       ▼
                                    ┌──────────────────────────────────────┐
                                    │ Consumidores de Eventos              │
                                    │  - UI: mostra progresso no terminal  │
                                    │  - Logs: tracing::info!              │
                                    │  - TaskStore: persiste estado        │
                                    └──────────────────────────────────────┘
```

**Estados de uma Task:**
```
Pending → Running → Completed
   ↓         ↓
Retry   Failed/Cancelled
```

---

## Fluxo 6: Requisição ao Hyscode Provider Service (SaaS)

```
┌─────────────┐         ┌─────────────────────────────────────────────────────┐
│  CLI Client │────────►│         Hyscode Provider Service (SaaS)             │
│             │  HTTPS  │                                                     │
└─────────────┘         │  ┌─────────────┐    ┌─────────────┐    ┌─────────┐ │
                        │  │ API Gateway │───►│   Router    │───►│ Cache?  │ │
                        │  │             │    │             │    │ (Redis) │ │
                        │  │ - Auth      │    │ - Model sel │    └────┬────┘ │
                        │  │ - Rate lim  │    │ - Fallback  │         │      │
                        │  │ - Logging   │    │ - Retry     │    Hit ▼      │
                        │  └─────────────┘    └──────┬──────┘    ┌─────┐    │
                        │                            │           │Return│   │
                        │                            ▼           └─────┘    │
                        │                     ┌─────────────┐               │
                        │                     │  Upstream   │               │
                        │                     │  Adapter    │               │
                        │                     └──────┬──────┘               │
                        │                            │                      │
                        │                            ▼                      │
                        │              ┌───────────────────────┐            │
                        │              │ Provedores Externos   │            │
                        │              │ - OpenAI              │            │
                        │              │ - Anthropic           │            │
                        │              │ - Outros              │            │
                        │              └───────────────────────┘            │
                        │                            │                      │
                        │                            ▼                      │
                        │              ┌───────────────────────┐            │
                        │              │   Billing / Usage     │            │
                        │              │   (PostgreSQL)        │            │
                        │              └───────────────────────┘            │
                        └─────────────────────────────────────────────────────┘
```

### Detalhes da Requisição

**Request:**
```http
POST /v1/chat/completions
Authorization: Bearer hsk_xxxxxxxxxxxxxxxx
Content-Type: application/json

{
  "model": "hyscode-smart",
  "messages": [...],
  "stream": true,
  "tools": [...]
}
```

**Roteamento Interno:**
```
1. API Gateway valida API Key (busca no PostgreSQL)
2. Verifica rate limit (Redis: sliding window)
3. Router resolve "hyscode-smart" para provedor+modelo real
   (ex: claude-3.5-sonnet @ Anthropic)
4. Verifica cache (Redis) para prompt idêntico
5. Encaminha request formatado para provedor upstream
6. Stream de resposta volta para cliente
7. Billing registra tokens de entrada/saída
```

---

## Fluxo 7: Streaming de Resposta

```
Provedor          HTTP Client              Engine                 UI
   │                   │                      │                      │
   │ ── SSE chunk ────►│                      │                      │
   │                   │ ── mpsc::Sender ────►│                      │
   │                   │                      │ ── parse markdown ──►│
   │                   │                      │                      │
   │ ── SSE chunk ────►│                      │                      │
   │                   │ ── mpsc::Sender ────►│                      │
   │                   │                      │ ── syntax highlight ─►│
   │                   │                      │                      │
   │ ── [DONE] ───────►│                      │                      │
   │                   │ ── close channel ──► │                      │
   │                   │                      │ ── final render ────►│
   │                   │                      │                      │
   │                   │                      │ ◄── user input ──────│ (modo REPL)
```

### Tipos de Evento SSE

| Evento | Origem | Ação na CLI |
|--------|--------|-------------|
| `content_block_delta` | Anthropic | Acumula texto, renderiza |
| `content_block_stop` | Anthropic | Finaliza bloco |
| `delta` (choices) | OpenAI | Acumula texto, renderiza |
| `[DONE]` | OpenAI | Finaliza stream |
| `tool_call` | Vários | Aciona Tool Dispatcher |

---

## Fluxo 8: Execução Segura de Ferramentas (PermissionManager)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Execução de Ferramenta                              │
│                                                                              │
│   Entrada: tool_call do modelo (ex: write_file /tmp/test.rs "fn main()...")│
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ 1. Validação de Schema                                             │   │
│   │    - Parâmetros obrigatórios presentes?                            │   │
│   │    - Tipos corretos?                                               │   │
│   │    └─► Inválido → retorna erro ao modelo                           │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ 2. PermissionManager (fail-closed)                                 │   │
│   │                                                                    │   │
│   │    ┌─────────────┐                                                 │   │
│   │    │ audit-only? │──Sim──► retorna "would write to..."            │   │
│   │    └──────┬──────┘                                                 │   │
│   │           │ Não                                                    │   │
│   │           ▼                                                        │   │
│   │    ┌─────────────┐                                                 │   │
│   │    │auto-approve │──Sim──► executa direto                         │   │
│   │    │   all?      │                                                 │   │
│   │    └──────┬──────┘                                                 │   │
│   │           │ Não                                                    │   │
│   │           ▼                                                        │   │
│   │    ┌─────────────┐                                                 │   │
│   │    │ auto-approve│──Sim──► executa (read-only)                     │   │
│   │    │   reads?    │                                                 │   │
│   │    └──────┬──────┘                                                 │   │
│   │           │ Não                                                    │   │
│   │           ▼                                                        │   │
│   │    ┌─────────────┐                                                 │   │
│   │    │  callback   │──► prompt interativo ou timeout                 │   │
│   │    │ confirm()   │                                                 │   │
│   │    └─────────────┘                                                 │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ 3. Sanitização de Inputs                                           │   │
│   │    - Path traversal check (../../etc/passwd)                       │   │
│   │    - Command injection check (rm -rf /)                            │   │
│   │    - Restrição ao working directory                                │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ 4. Execução                                                        │   │
│   │    - read_file: tokio::fs::read_to_string                          │   │
│   │    - write_file: tokio::fs::write + cria dirs pai                  │   │
│   │    - execute_cmd: tokio::process::Command (com timeout)            │   │
│   │    - search_code: walkdir + regex (spawn_blocking)                 │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ 5. Retorno                                                         │   │
│   │    - Sucesso: ToolResult::success → modelo                         │   │
│   │    - Erro: ToolResult::error → modelo tenta corrigir               │   │
│   │    - Log: tracing::info! + audit log                               │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```
