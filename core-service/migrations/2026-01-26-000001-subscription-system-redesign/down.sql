-- ============================================================================
-- Subscription System Redesign Migration - Rollback
-- ============================================================================

-- ============================================================================
-- 1. Drop new indexes
-- ============================================================================
DROP INDEX IF EXISTS idx_subscriptions_assignment_status;
DROP INDEX IF EXISTS idx_subscriptions_source_type;
DROP INDEX IF EXISTS idx_subscriptions_auto_selected;
DROP INDEX IF EXISTS idx_subscriptions_created_at;

-- ============================================================================
-- 2. Remove new constraints and columns from subscription_conflicts
-- ============================================================================
ALTER TABLE subscription_conflicts
DROP CONSTRAINT IF EXISTS subscription_conflicts_subscription_id_fkey;

-- ============================================================================
-- 3. Remove new columns from subscriptions
-- ============================================================================
ALTER TABLE subscriptions
DROP COLUMN IF EXISTS source_type,
DROP COLUMN IF EXISTS assignment_status,
DROP COLUMN IF EXISTS assigned_at,
DROP COLUMN IF EXISTS auto_selected;

-- ============================================================================
-- 4. Rename source_url back to rss_url
-- ============================================================================
ALTER TABLE subscriptions
RENAME COLUMN source_url TO rss_url;

-- ============================================================================
-- 5. Rename subscriptions table back to rss_subscriptions
-- ============================================================================
ALTER TABLE subscriptions
RENAME TO rss_subscriptions;

-- ============================================================================
-- 6. Restore old constraint
-- ============================================================================
ALTER TABLE rss_subscriptions
DROP CONSTRAINT IF EXISTS subscriptions_fetcher_id_source_url_key;

ALTER TABLE rss_subscriptions
ADD CONSTRAINT rss_subscriptions_fetcher_id_rss_url_key UNIQUE (fetcher_id, rss_url);

-- ============================================================================
-- 7. Restore old foreign key constraint
-- ============================================================================
ALTER TABLE subscription_conflicts
ADD CONSTRAINT subscription_conflicts_subscription_id_fkey
FOREIGN KEY (subscription_id) REFERENCES rss_subscriptions(subscription_id) ON DELETE CASCADE;

-- ============================================================================
-- 8. Remove priority column from fetcher_modules
-- ============================================================================
ALTER TABLE fetcher_modules
DROP COLUMN IF EXISTS priority;
