CREATE TABLE webhooks (
    webhook_id   SERIAL PRIMARY KEY,
    name         VARCHAR(255) NOT NULL,
    url          TEXT NOT NULL,
    payload_template TEXT NOT NULL DEFAULT '{"download_id": {{download_id}}, "anime_title": "{{anime_title}}", "episode_no": {{episode_no}}}',
    is_active    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at   TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMP NOT NULL DEFAULT NOW()
);
