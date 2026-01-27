-- Rollback: Remove base_url column and index
DROP INDEX IF EXISTS idx_fetcher_modules_base_url;

ALTER TABLE fetcher_modules
DROP COLUMN base_url;
