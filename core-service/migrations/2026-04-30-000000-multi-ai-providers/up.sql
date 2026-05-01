CREATE TABLE ai_providers (
    id            SERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    provider_kind TEXT NOT NULL,
    base_url      TEXT NOT NULL DEFAULT '',
    api_key       TEXT NOT NULL DEFAULT '',
    model_name    TEXT NOT NULL DEFAULT '',
    max_tokens    INT  NOT NULL DEFAULT 4096,
    response_format_mode TEXT NOT NULL DEFAULT 'non_strict',
    is_enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    priority      INT NOT NULL DEFAULT 0,
    created_at    TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ai_providers_enabled_priority
    ON ai_providers (is_enabled, priority);

INSERT INTO ai_providers
    (name, provider_kind, base_url, api_key, model_name,
     max_tokens, response_format_mode, is_enabled, priority)
SELECT 'Default', 'openai_compatible', base_url, api_key, model_name,
       max_tokens, response_format_mode, TRUE, 0
FROM ai_settings;

DROP TABLE ai_settings;
