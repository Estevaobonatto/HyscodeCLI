# ADR-002: Arquitetura da CLI em Rust

> **Status:** Aceito  
> **Data:** 2026-04-22  
> **Decisores:** Arquiteto, Tech Lead

## Contexto

Precisamos escolher a linguagem e arquitetura para a CLI do Hyscode. Os requisitos incluem:

- Performance de inicialização rápida (< 200ms)
- Binário único, fácil de distribuir
- Suporte a async I/O (múltiplas requisições HTTP concorrentes)
- Interface rica no terminal (streaming, cores, tabelas)
- Segurança na manipulação de credenciais
- Cross-platform (Windows, macOS, Linux)

## Decisão

**Rust** será a linguagem da CLI, organizada como um **workspace de crates** com arquitetura hexagonal/ports-and-adapters.

### Justificativa da Linguagem

| Critério | Rust | Go | Python | Node.js |
|----------|------|-----|--------|---------|
| Performance | ⭐⭐⭐ | ⭐⭐ | ⭐ | ⭐ |
| Binário único | ⭐⭐⭐ | ⭐⭐⭐ | ⭐ | ⭐ |
| Cross-compile | ⭐⭐⭐ | ⭐⭐ | ⭐ | ⭐ |
| Async nativo | ⭐⭐⭐ (tokio) | ⭐⭐ | ⭐⭐ (asyncio) | ⭐⭐⭐ |
| Terminal UI | ⭐⭐⭐ (ratatui) | ⭐⭐ | ⭐⭐ | ⭐⭐ |
| Segurança | ⭐⭐⭐ | ⭐⭐ | ⭐ | ⭐ |
| Ecossistema CLI | ⭐⭐⭐ (clap) | ⭐⭐ | ⭐⭐ | ⭐⭐ |

### Estrutura de Workspace

```
hyscode/
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── hyscode-cli/              # Binário principal
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs           # Entry point, parse args, dispatch
│   │
│   ├── hyscode-core/             # Domínio: tipos, traits, lógica pura
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── models/           # ChatRequest, Message, Provider, etc
│   │       ├── traits/           # Provider trait, Tool trait
│   │       └── error.rs          # Error types (thiserror)
│   │
│   ├── hyscode-engine/           # Orquestração
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── conversation.rs   # ConversationManager
│   │       ├── context.rs        # ContextBuilder
│   │       ├── agent.rs          # Agent loop
│   │       └── token.rs          # TokenManager
│   │
│   ├── hyscode-provider/         # Adapters de provedores
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs       # ProviderRegistry
│   │       ├── models.rs         # Tipos normalizados
│   │       ├── http/             # Cliente HTTP compartilhado
│   │       └── adapters/         # Implementações específicas
│   │
│   ├── hyscode-tools/            # Ferramentas do agente
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs       # ToolRegistry
│   │       ├── read_file.rs
│   │       ├── write_file.rs
│   │       ├── execute_command.rs
│   │       └── search_code.rs
│   │
│   ├── hyscode-ui/               # Interface do terminal
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── stream.rs         # Renderização de stream
│   │       ├── tui.rs            # Interface interativa (ratatui)
│   │       └── markdown.rs       # Parser/Renderer de markdown
│   │
│   ├── hyscode-config/           # Configurações e secrets
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── file.rs           # Leitura/escrita de config TOML
│   │       ├── keyring.rs        # Integração com keyring do SO
│   │       └── env.rs            # Variáveis de ambiente
│   │
│   └── hyscode-macros/           # Procedural macros
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs            # Derive macros internos
```

### Dependências entre Crates

```
hyscode-cli
├── hyscode-engine
│   ├── hyscode-core
│   ├── hyscode-provider
│   │   └── hyscode-core
│   └── hyscode-tools
│       └── hyscode-core
├── hyscode-ui
│   └── hyscode-core
└── hyscode-config
    └── hyscode-core
```

**Regra:** Dependências apontam sempre para baixo (camadas internas). `hyscode-core` não depende de nenhum outro crate interno.

### Padrão Arquitetural: Ports and Adapters (Hexagonal)

```
┌─────────────────────────────────────────────────────────┐
│                    Driving Adapters                      │
│   ┌──────────┐  ┌──────────┐  ┌──────────────────────┐ │
│   │ CLI Args │  │   TUI    │  │     CI/Scripts       │ │
│   │ (clap)   │  │(ratatui) │  │   (JSON mode)        │ │
│   └────┬─────┘  └────┬─────┘  └──────────┬───────────┘ │
└────────┼─────────────┼───────────────────┼─────────────┘
         │             │                   │
         └─────────────┴───────────────────┘
                         │
         ┌───────────────▼────────────────┐
         │         Application             │
         │      ┌──────────────────┐      │
         │      │   Engine (app)   │      │
         │      └──────────────────┘      │
         └───────────────┬────────────────┘
                         │
         ┌───────────────▼────────────────┐
         │           Domain                │
         │      ┌──────────────────┐      │
         │      │   Core (domain)  │      │
         │      │  - Models        │      │
         │      │  - Traits (ports)│      │
         │      └──────────────────┘      │
         └───────────────┬────────────────┘
                         │
         ┌───────────────▼────────────────┐
         │      Driven Adapters            │
         │  ┌────────┐  ┌──────────────┐  │
         │  │Provider│  │   Tool Impl  │  │
         │  │ Adapters│  │  (filesystem)│  │
         │  └────────┘  └──────────────┘  │
         │  ┌────────┐  ┌──────────────┐  │
         │  │ Config │  │    Git       │  │
         │  │ (TOML) │  │  (git2/lib)  │  │
         │  └────────┘  └──────────────┘  │
         └─────────────────────────────────┘
```

### Async Runtime

- **Tokio** como runtime async principal
- **tokio::sync::mpsc** para streaming de respostas entre provider e UI
- **tokio::task** para operações concorrentes (I/O, parsing)
- **tokio::time::timeout** para timeouts em requisições

### Tratamento de Erros

| Camada | Crate | Uso |
|--------|-------|-----|
| Domínio | `thiserror` | Erros tipados com contexto |
| Aplicação | `anyhow` | Propagação com context |
| CLI | `eyre` / `anyhow` | Report amigável ao usuário |

Exemplo:
```rust
// hyscode-core/src/error.rs
#[derive(thiserror::Error, Debug)]
pub enum ProviderError {
    #[error("provedor {0} não está configurado")]
    NotConfigured(String),
    #[error("credenciais inválidas para {0}")]
    InvalidCredentials(String),
    #[error("rate limit excedido, tente em {0}s")]
    RateLimited(u64),
    #[error("timeout na requisição")]
    Timeout,
    #[error("erro HTTP {status}: {message}")]
    Http { status: u16, message: String },
}
```

## Consequências

### Positivas
- **Performance:** Rust entrega performance próxima de C com segurança de memória
- **Distribuição:** Binário único, sem runtime externo
- **Confiabilidade:** Type system previne grandes classes de bugs
- **Ecossistema:** crates de alta qualidade para CLI (clap, ratatui, tokio)

### Negativas
- **Curva de aprendizado:** Time precisa de familiaridade com Rust
- **Tempo de compilação:** Builds mais lentos que Go/Python
- **Async complexity:** Lifetime + async pode ser desafiador

## Notas de Implementação

1. **Feature flags por provedor:** Cada adapter é uma feature Cargo para reduzir tamanho do binário
   ```toml
   [features]
   default = ["openai", "anthropic"]
   openai = []
   anthropic = []
   copilot = []
   openrouter = []
   hyscode = []
   ```

2. **Binários otimizados:**
   ```toml
   [profile.release]
   opt-level = 3
   lto = true
   strip = true
   codegen-units = 1
   ```

3. **Cross-compilation:** Usar `cross` ou GitHub Actions com targets:
   - `x86_64-unknown-linux-gnu`
   - `x86_64-unknown-linux-musl`
   - `aarch64-unknown-linux-gnu`
   - `x86_64-pc-windows-msvc`
   - `aarch64-apple-darwin`
   - `x86_64-apple-darwin`
