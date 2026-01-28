-- Drop foreign key constraints that depend on old tables
ALTER TABLE subscriptions DROP CONSTRAINT IF EXISTS rss_subscriptions_fetcher_id_fkey;

-- Drop indexes from old tables
DROP INDEX IF EXISTS idx_fetcher_modules_base_url;
DROP INDEX IF EXISTS idx_downloader_modules_base_url;
DROP INDEX IF EXISTS idx_viewer_modules_base_url;

-- Drop old tables
DROP TABLE IF EXISTS fetcher_modules;
DROP TABLE IF EXISTS downloader_modules;
DROP TABLE IF EXISTS viewer_modules;
