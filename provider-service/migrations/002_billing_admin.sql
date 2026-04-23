-- Provider Service — PostgreSQL Migrations
-- Migration 002: Billing, dashboard, admin

-- Adiciona role aos usuários
ALTER TABLE users ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';

-- Planos de assinatura
CREATE TABLE IF NOT EXISTS subscription_plans (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name                TEXT        NOT NULL,
    tier                TEXT        NOT NULL,   -- free | pro | enterprise
    monthly_limit_tokens BIGINT     NOT NULL DEFAULT 0,
    monthly_price_cents  INTEGER     NOT NULL DEFAULT 0,
    features            TEXT[]      NOT NULL DEFAULT '{}',
    is_active           BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER subscription_plans_updated_at BEFORE UPDATE ON subscription_plans
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Assinaturas dos usuários
CREATE TABLE IF NOT EXISTS subscriptions (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan_id     UUID        NOT NULL REFERENCES subscription_plans(id),
    status      TEXT        NOT NULL DEFAULT 'active', -- active | cancelled | expired
    started_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ends_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_subscriptions_user_id ON subscriptions(user_id);
CREATE TRIGGER subscriptions_updated_at BEFORE UPDATE ON subscriptions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Preços por modelo
CREATE TABLE IF NOT EXISTS pricing_per_model (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    model_alias             TEXT        NOT NULL UNIQUE,
    provider                TEXT        NOT NULL,
    input_price_per_1k      BIGINT      NOT NULL DEFAULT 0,  -- em centavos de centavo (1/10000 USD)
    output_price_per_1k     BIGINT      NOT NULL DEFAULT 0,
    currency                TEXT        NOT NULL DEFAULT 'USD',
    is_active               BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER pricing_per_model_updated_at BEFORE UPDATE ON pricing_per_model
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Registros de billing mensal
CREATE TABLE IF NOT EXISTS billing_records (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    period_start    DATE        NOT NULL,
    period_end      DATE        NOT NULL,
    total_tokens    BIGINT      NOT NULL DEFAULT 0,
    total_requests  INTEGER     NOT NULL DEFAULT 0,
    total_cost_cents BIGINT     NOT NULL DEFAULT 0,  -- em centavos (1/100 USD)
    status          TEXT        NOT NULL DEFAULT 'open', -- open | closed | paid
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, period_start, period_end)
);

CREATE INDEX idx_billing_records_user_id ON billing_records(user_id);
CREATE INDEX idx_billing_records_period ON billing_records(period_start DESC, period_end DESC);
CREATE TRIGGER billing_records_updated_at BEFORE UPDATE ON billing_records
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Alertas de limite de gasto
CREATE TABLE IF NOT EXISTS usage_alerts (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    alert_type  TEXT        NOT NULL, -- quota_80 | quota_100 | billing_threshold
    message     TEXT        NOT NULL,
    is_read     BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_usage_alerts_user_id ON usage_alerts(user_id);

-- Monitoramento de saúde dos provedores upstream
CREATE TABLE IF NOT EXISTS provider_health (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    provider        TEXT        NOT NULL,
    checked_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    latency_ms      INTEGER,
    status          TEXT        NOT NULL, -- healthy | degraded | down
    error_message   TEXT,
    endpoint_url    TEXT
);

CREATE INDEX idx_provider_health_provider ON provider_health(provider);
CREATE INDEX idx_provider_health_checked_at ON provider_health(checked_at DESC);

-- Seed planos padrão
INSERT INTO subscription_plans (name, tier, monthly_limit_tokens, monthly_price_cents, features)
VALUES
    ('Free',    'free',       500000,   0,      '{"chat","models"}'),
    ('Pro',     'pro',       5000000,  999,     '{"chat","models","priority"}'),
    ('Enterprise', 'enterprise', 50000000, 4999, '{"chat","models","priority","dedicated"}')
ON CONFLICT DO NOTHING;

-- Função para reset mensal de quotas
CREATE OR REPLACE FUNCTION reset_monthly_quotas()
RETURNS void AS $$
BEGIN
    UPDATE usage_quotas
    SET monthly_tokens = 0,
        reset_at = date_trunc('month', NOW()) + INTERVAL '1 month'
    WHERE reset_at <= NOW();
END;
$$ LANGUAGE plpgsql;
