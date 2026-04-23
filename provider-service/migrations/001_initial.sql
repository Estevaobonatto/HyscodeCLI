-- Provider Service — PostgreSQL Migrations
-- Migration 001: Initial schema

-- Tabela de usuários
CREATE TABLE IF NOT EXISTS users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    display_name  TEXT,
    tier          TEXT        NOT NULL DEFAULT 'free',   -- free | pro | enterprise
    is_active     BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tabela de API keys
CREATE TABLE IF NOT EXISTS api_keys (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_hash      TEXT        NOT NULL UNIQUE,        -- SHA-256(key)
    key_prefix    TEXT        NOT NULL,               -- "hsk_..." primeiros 8 chars
    label         TEXT,
    scopes        TEXT[]      NOT NULL DEFAULT '{}',  -- ["chat", "models"]
    rate_limit_rpm INTEGER    NOT NULL DEFAULT 60,
    is_active     BOOLEAN     NOT NULL DEFAULT TRUE,
    last_used_at  TIMESTAMPTZ,
    expires_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_user_id  ON api_keys(user_id);

-- Tabela de log de requisições
CREATE TABLE IF NOT EXISTS requests_log (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    api_key_id      UUID        REFERENCES api_keys(id),
    user_id         UUID        REFERENCES users(id),
    model           TEXT        NOT NULL,
    upstream_provider TEXT      NOT NULL,
    prompt_tokens   INTEGER     NOT NULL DEFAULT 0,
    completion_tokens INTEGER   NOT NULL DEFAULT 0,
    total_tokens    INTEGER     NOT NULL DEFAULT 0,
    latency_ms      INTEGER,
    status_code     SMALLINT    NOT NULL,
    error_message   TEXT,
    requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_requests_log_user_id    ON requests_log(user_id);
CREATE INDEX idx_requests_log_requested_at ON requests_log(requested_at DESC);

-- Tabela de quotas de uso
CREATE TABLE IF NOT EXISTS usage_quotas (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID        NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    monthly_tokens  BIGINT      NOT NULL DEFAULT 0,
    monthly_limit   BIGINT      NOT NULL DEFAULT 1000000,
    reset_at        TIMESTAMPTZ NOT NULL DEFAULT (date_trunc('month', NOW()) + INTERVAL '1 month'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Índices e trigger de updated_at
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER usage_quotas_updated_at BEFORE UPDATE ON usage_quotas
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
