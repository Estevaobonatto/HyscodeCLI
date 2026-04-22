# ADR-004: Armazenamento Local, Histórico e RAG

> **Status:** Aceito  
> **Data:** 2026-04-22  
> **Decisores:** Arquiteto, Tech Lead

## Contexto

A CLI precisa armazenar dados localmente:

1. **Histórico de conversas:** Mensagens passadas para contexto e replay
2. **Configurações:** API keys, preferências, providers
3. **Cache:** Respostas cacheadas (performance)
4. **Índice RAG:** Vetores semânticos do código do projeto (fase futura)

Requisitos:
- Funcionar completamente offline
- Sem servidor local
- Leve e rápido (integrado ao binário)
- Portátil entre sistemas

## Decisão

### Armazenamento: SQLite via `sqlx`

**SQLite** será o banco de dados local para histórico, cache e índice RAG.

**Justificativa:**
- Arquivo único, fácil de backup e migração
- `sqlx` oferece queries assíncronas type-safe
- Extensão `sqlite-vss` ou `sqlite-vec` para vetores (RAG)
- Zero-config — não requer daemon

**Localização dos arquivos:**

| Propósito | Caminho |
|-----------|---------|
| Histórico | `~/.local/share/hyscode/history.db` |
| Cache | `~/.cache/hyscode/cache.db` |
| Config | `~/.config/hyscode/config.toml` |
| Índice RAG | `.hyscode/index.db` (por projeto) |

### Configurações: TOML + Keyring do SO

- **Configurações não-sensíveis:** `~/.config/hyscode/config.toml`
- **API Keys:** Keyring do SO (nunca em disco em plaintext)
- **Variáveis de ambiente:** Sobrepõem configuração de arquivo

### Diretórios XDG (cross-platform via `dirs` crate)

```rust
// Usa dirs crate para respeitar convenções de cada SO:
// Linux: $XDG_CONFIG_HOME (~/.config) e $XDG_DATA_HOME (~/.local/share)
// macOS: ~/Library/Application Support/
// Windows: %APPDATA%\

let config_dir = dirs::config_dir().unwrap().join("hyscode");
let data_dir = dirs::data_dir().unwrap().join("hyscode");
let cache_dir = dirs::cache_dir().unwrap().join("hyscode");
```

### RAG (Retrieval-Augmented Generation) — Fase 6

Para o índice semântico de código:
- **Embeddings:** Gerados via provedor configurado ou modelo local (Ollama)
- **Armazenamento:** `sqlite-vec` (extensão SQLite para vetores)
- **Chunking:** Arquivos de código divididos em funções/classes
- **Atualização:** Incremental baseado em hash SHA256 dos arquivos

**Schema do índice RAG:**
```sql
CREATE VIRTUAL TABLE code_chunks USING vec0(
    embedding float[1536]  -- dimensão depende do modelo de embedding
);

CREATE TABLE code_metadata (
    id          INTEGER PRIMARY KEY,
    file_path   TEXT NOT NULL,
    chunk_start INTEGER,
    chunk_end   INTEGER,
    content     TEXT NOT NULL,
    file_hash   TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
);
```

## Migração de Schema

- Versão do schema armazenada em tabela `schema_version`
- Migrations aplicadas automaticamente ao iniciar
- Migrations versionadas em `crates/hyscode-engine/migrations/`
- Rollback manual documentado por migração

## Consequências

### Positivas
- **Sem dependências externas:** Funciona em qualquer máquina
- **Performance:** SQLite é extremamente rápido para workloads locais
- **Portabilidade:** Arquivo pode ser copiado/sincronizado
- **RAG local:** Não precisa enviar código para servidores externos

### Negativas
- **Sem sync entre máquinas:** Histórico é local por padrão
- **Tamanho do binário:** `sqlx` e `sqlite-vec` adicionam ~5MB
- **Concorrência limitada:** SQLite não é ideal para múltiplas instâncias simultâneas

## Alternativas Consideradas

| Alternativa | Motivo do descarte |
|-------------|-------------------|
| PostgreSQL local | Requer instalação separada; complexo para CLI |
| sled (embedded KV) | Sem suporte a queries complexas; sem extensão de vetores |
| JSON files | Lento para grandes históricos; difícil de fazer queries |
| LanceDB | Boa para vetores, mas dependency pesada; menos madura |
