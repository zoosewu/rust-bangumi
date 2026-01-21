CREATE TABLE cron_logs (
  log_id SERIAL PRIMARY KEY,
  fetcher_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL CHECK (status IN ('success', 'failed')),
  error_message TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 1,
  executed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cron_logs_fetcher_type ON cron_logs(fetcher_type);
CREATE INDEX idx_cron_logs_executed_at ON cron_logs(executed_at);
