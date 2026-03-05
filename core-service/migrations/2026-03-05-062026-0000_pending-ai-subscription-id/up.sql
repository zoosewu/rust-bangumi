ALTER TABLE pending_ai_results
    ADD COLUMN subscription_id INT REFERENCES subscriptions(subscription_id) ON DELETE SET NULL;

CREATE INDEX idx_pending_ai_results_subscription_id
    ON pending_ai_results(subscription_id);
