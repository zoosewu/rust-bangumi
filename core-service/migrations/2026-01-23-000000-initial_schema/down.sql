-- ============================================================================
-- Rollback Initial Database Schema
-- ============================================================================

DROP INDEX IF EXISTS idx_subscription_conflicts_resolution_status;
DROP INDEX IF EXISTS idx_subscription_conflicts_subscription_id;
DROP TABLE IF EXISTS subscription_conflicts;
DROP TABLE IF EXISTS rss_subscriptions;
DROP TABLE IF EXISTS fetcher_modules;
DROP INDEX IF EXISTS idx_cron_logs_fetcher_type;
DROP TABLE IF EXISTS cron_logs;
DROP TABLE IF EXISTS downloads;
DROP TABLE IF EXISTS filter_rules;
DROP TABLE IF EXISTS anime_links;
DROP TABLE IF EXISTS subtitle_groups;
DROP TABLE IF EXISTS anime_series;
DROP TABLE IF EXISTS animes;
DROP TABLE IF EXISTS seasons;
