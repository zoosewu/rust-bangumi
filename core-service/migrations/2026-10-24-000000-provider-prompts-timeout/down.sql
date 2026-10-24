ALTER TABLE ai_providers
    DROP COLUMN timeout_secs,
    DROP COLUMN parser_prompt,
    DROP COLUMN filter_prompt;
