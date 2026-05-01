CREATE TABLE ai_settings (
    id         SERIAL PRIMARY KEY,
    base_url   TEXT NOT NULL DEFAULT '',
    api_key    TEXT NOT NULL DEFAULT '',
    model_name TEXT NOT NULL DEFAULT 'gpt-4o-mini',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    max_tokens INT NOT NULL DEFAULT 4096,
    response_format_mode TEXT NOT NULL DEFAULT 'non_strict'
);

INSERT INTO ai_settings (base_url, api_key, model_name, max_tokens, response_format_mode)
SELECT base_url, api_key, model_name, max_tokens, response_format_mode
FROM ai_providers
ORDER BY priority ASC, id ASC
LIMIT 1;

INSERT INTO ai_settings (base_url, api_key, model_name)
SELECT '', '', 'gpt-4o-mini'
WHERE NOT EXISTS (SELECT 1 FROM ai_settings);

DROP TABLE ai_providers;
