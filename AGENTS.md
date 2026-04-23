# AGENTS.md — Guia para Agentes de Codificação

> Este arquivo instrui agentes de IA (como HyscodeCLI, Claude, GitHub Copilot)
> sobre convenções, restrições e fluxos de trabalho deste repositório.

## Nunca fazer TODOs — implemente de forma completa

Quando solicitado a implementar uma feature, **não deixe `todo!()` stubs** em código
que deve estar funcional. Só use `todo!()` em módulos explicitamente marcados como
esqueleto de fase futura.

## Estrutura do Projeto

```
hyscode/                          # Workspace Rust
├── Cargo.toml                    # Workspace com resolver = "2"
├── crates/
│   ├── hyscode-cli/              # Binário: comandos CLI (clap)
│   ├── hyscode-core/             # Domínio: types, traits, errors
│   ├── hyscode-engine/           # Orquestração: agent loop, context, tokens
│   ├── hyscode-provider/         # Adapters de provedores de LLM
│   ├── hyscode-tools/            # Ferramentas do agente (fs, shell, git)
│   ├── hyscode-ui/               # Interface terminal (ratatui, streaming)
│   ├── hyscode-config/           # Config TOML, keyring, env vars
│   └── hyscode-macros/           # Procedural macros internas
├── docs/
│   ├── requirements/REQUISITOS.md
│   ├── architecture/
│   │   ├── ARQUITETURA.md
│   │   ├── MODELOS_C4.md
│   │   ├── FLUXOS.md
│   │   └── MODELOS_DADOS.md
│   ├── adr/
│   │   ├── ADR-001-provider-abstraction.md
│   │   ├── ADR-002-cli-architecture.md
│   │   └── ADR-003-provider-service.md
│   ├── api/provider-api.md
│   └── provider-service/ARQUITETURA_SERVICO.md
└── provider-service/             # (Fase 4) Código do SaaS
```

## Convenções de Código Rust

### Módulos e Crates
- Cada crate tem responsabilidade única (Single Responsibility)
- `hyscode-core` **não depende de nenhum crate interno** — é a base
- Dependências sempre apontam para baixo: `cli → engine → core`
- Feature flags para cada provedor: `#[cfg(feature = "openai")]`

### Tratamento de Erros
- **Domínio** (`hyscode-core`): Use `thiserror` com tipos enumerados
- **Aplicação** (`hyscode-engine`, `hyscode-provider`): Use `anyhow::Result`
- **CLI** (`hyscode-cli`): Use `eyre` para mensagens amigáveis ao usuário
- **Nunca** use `.unwrap()` em código de produção; use `?` ou `expect()` com mensagem clara
- **Nunca** deixe `panic!` em paths de execução normal

### Async
- Todo I/O deve ser `async` via `tokio`
- Use `tokio::spawn` para tarefas independentes
- Use `tokio::sync::mpsc` para streaming entre tasks
- Use `tokio::time::timeout` para operações com limite de tempo

### Tipos e Ownership
- Prefira `&str` sobre `String` em parâmetros de função
- Use `Arc<dyn Trait>` para compartilhar provedores e ferramentas
- Implemente `Clone` apenas quando necessário (não automaticamente)
- Use `Cow<'_, str>` quando o texto pode ser owned ou borrowed

### Formatação e Linting
- `cargo fmt` deve passar sem modificações
- `cargo clippy -- -D warnings` deve passar sem warnings
- Documente todas as APIs públicas com `///` rustdoc
- Use `#[allow(...)]` apenas quando absolutamente necessário, com comentário explicando

## Convenções de Commit

Siga Conventional Commits:
```
feat(provider): adiciona adapter para Z.ai
fix(engine): corrige truncamento de mensagens no TokenEstimator
docs(adr): adiciona ADR-004 para armazenamento local
refactor(core): extrai ToolResult para módulo separado
test(provider): adiciona testes de integração para OpenAI adapter
```

## Convenções de Testes

### Unitários (`#[cfg(test)]` no mesmo arquivo)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimator_fits() { ... }

    #[tokio::test]
    async fn test_read_file_tool() { ... }
}
```

### Integração (`tests/` na raiz do crate)
```rust
// crates/hyscode-provider/tests/openai_integration.rs
// Requer: OPENAI_API_KEY no ambiente
// Execute: cargo test --features openai -- --ignored openai_
```

### Mocks de Provedor
- Use `wiremock` para mockar HTTP em testes
- Use `mockall` para mockar o trait `Provider`
- Teste SEMPRE o caminho de erro (timeout, credenciais inválidas, etc.)

## Comandos Úteis

```bash
# Build de todos os crates
cargo build --workspace

# Testes sem features opcionais
cargo test --workspace

# Testes com todos os provedores
cargo test --workspace --all-features

# Linting
cargo clippy --workspace -- -D warnings

# Formatação
cargo fmt --all

# Verificar dependências vulneráveis
cargo audit

# Build de release otimizado
cargo build --release --workspace

# Executar a CLI em desenvolvimento
cargo run -p hyscode-cli -- chat "Olá"
```

## Adicionando um Novo Provedor

1. Crie `crates/hyscode-provider/src/adapters/<nome>.rs`
2. Adicione feature no `Cargo.toml` de `hyscode-provider`
3. Adicione entrada em `crates/hyscode-provider/src/adapters/mod.rs`
4. Implemente o trait `Provider` do `hyscode-core`
5. Registre no `ProviderRegistry` via feature flag
6. Adicione entrada em `crates/hyscode-config/src/env.rs`
7. Documente no `docs/architecture/ARQUITETURA.md`
8. Crie ADR se houver decisão arquitetural relevante

## Restrições de Segurança

- **NUNCA** logar API keys (nem em `tracing::debug!`)
- **NUNCA** escrever API keys em arquivos de config em plaintext — use keyring
- **SEMPRE** validar e sanitizar paths antes de operações de filesystem
- **SEMPRE** verificar se um path está dentro do working directory (prevenir path traversal)
- Comandos shell devem ter timeout máximo (padrão 30s, máximo 300s)
- Nunca executar comandos construídos com interpolação de strings de user input sem sanitização

## Fase Atual e Roadmap

| Fase | Status | Descrição |
|------|--------|-----------|
| 1 | Concluída | Planejamento e arquitetura |
| 2 | Concluída | Core da CLI: config, providers básicos, chat |
| 3 | Próxima | Engine completo: agent loop, tools, streaming |
| 4 | Concluída | Provider Service SaaS |
| 5 | Concluída | Billing, dashboard, admin |
| 6 | Futura | RAG local, TUI avançada, plugins |

## Provider Service (Fase 4+)

O código do Provider Service (SaaS) ficará em `provider-service/` na raiz.
Stack: Rust (axum) + PostgreSQL + Redis.
Documentação: `docs/provider-service/` e `docs/api/`.
