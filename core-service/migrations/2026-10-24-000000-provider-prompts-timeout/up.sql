-- 每個 AI provider 自己的請求逾時，以及模型層的固定 prompt（NULL = 沿用全域）
ALTER TABLE ai_providers
    ADD COLUMN timeout_secs INTEGER NOT NULL DEFAULT 60 CHECK (timeout_secs BETWEEN 5 AND 600),
    ADD COLUMN parser_prompt TEXT,
    ADD COLUMN filter_prompt TEXT;
