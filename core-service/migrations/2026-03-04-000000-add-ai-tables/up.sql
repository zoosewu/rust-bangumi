CREATE TABLE ai_settings (
    id         SERIAL PRIMARY KEY,
    base_url   TEXT NOT NULL DEFAULT '',
    api_key    TEXT NOT NULL DEFAULT '',
    model_name TEXT NOT NULL DEFAULT 'gpt-4o-mini',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);
-- 確保只有一筆記錄
INSERT INTO ai_settings (base_url, api_key, model_name) VALUES ('', '', 'gpt-4o-mini');

CREATE TABLE ai_prompt_settings (
    id                   SERIAL PRIMARY KEY,
    fixed_parser_prompt  TEXT,
    fixed_filter_prompt  TEXT,
    custom_parser_prompt TEXT,
    custom_filter_prompt TEXT,
    created_at           TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP NOT NULL DEFAULT NOW()
);
-- 確保只有一筆記錄（預設值由應用層 revert 寫入）
INSERT INTO ai_prompt_settings DEFAULT VALUES;

CREATE TABLE pending_ai_results (
    id                 SERIAL PRIMARY KEY,
    result_type        TEXT NOT NULL CHECK (result_type IN ('parser', 'filter')),
    source_title       TEXT NOT NULL,
    generated_data     JSONB,
    status             TEXT NOT NULL DEFAULT 'generating'
                           CHECK (status IN ('generating', 'pending', 'confirmed', 'failed')),
    error_message      TEXT,
    raw_item_id        INT REFERENCES raw_anime_items(item_id) ON DELETE SET NULL,
    used_fixed_prompt  TEXT NOT NULL DEFAULT '',
    used_custom_prompt TEXT,
    expires_at         TIMESTAMP,
    created_at         TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMP NOT NULL DEFAULT NOW()
);
