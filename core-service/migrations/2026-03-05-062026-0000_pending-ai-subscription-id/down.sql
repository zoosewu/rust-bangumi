DROP INDEX IF EXISTS idx_pending_ai_results_subscription_id;
ALTER TABLE pending_ai_results DROP COLUMN IF EXISTS subscription_id;
