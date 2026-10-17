-- AI 請求同時送出的數量（設定頁可調）。預設 1：provider 每分鐘 token 額度小時，一次一個最不容易撞上限流
ALTER TABLE ai_prompt_settings
    ADD COLUMN max_concurrent_requests INTEGER NOT NULL DEFAULT 1
    CHECK (max_concurrent_requests BETWEEN 1 AND 10);
