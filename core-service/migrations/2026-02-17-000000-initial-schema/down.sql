-- Drop all tables in reverse dependency order
DROP TABLE IF EXISTS downloader_capabilities;
DROP TABLE IF EXISTS downloads;
DROP TABLE IF EXISTS anime_link_conflicts;
DROP TABLE IF EXISTS anime_links;
DROP TABLE IF EXISTS raw_anime_items;
DROP TABLE IF EXISTS title_parsers;
DROP TABLE IF EXISTS filter_rules;
DROP INDEX IF EXISTS idx_cron_logs_fetcher_type;
DROP TABLE IF EXISTS cron_logs;
DROP TABLE IF EXISTS subscription_conflicts;
DROP TABLE IF EXISTS subscriptions;
DROP TABLE IF EXISTS service_modules;
DROP TABLE IF EXISTS subtitle_groups;
DROP TABLE IF EXISTS anime_series;
DROP TABLE IF EXISTS animes;
DROP TABLE IF EXISTS seasons;

DROP TYPE IF EXISTS filter_target_type;
DROP TYPE IF EXISTS parser_source_type;
DROP TYPE IF EXISTS module_type;
