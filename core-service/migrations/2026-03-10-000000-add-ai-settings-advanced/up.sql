ALTER TABLE ai_settings
    ADD COLUMN max_tokens         INT  NOT NULL DEFAULT 4096,
    ADD COLUMN response_format_mode TEXT NOT NULL DEFAULT 'strict'
        CHECK (response_format_mode IN ('strict', 'non_strict', 'inject_schema'));
