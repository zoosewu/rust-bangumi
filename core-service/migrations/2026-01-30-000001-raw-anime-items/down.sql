-- 移除 anime_links 的 raw_item_id
DROP INDEX IF EXISTS idx_anime_links_raw_item;
ALTER TABLE anime_links DROP COLUMN IF EXISTS raw_item_id;

-- 移除 raw_anime_items
DROP INDEX IF EXISTS idx_raw_items_created;
DROP INDEX IF EXISTS idx_raw_items_subscription;
DROP INDEX IF EXISTS idx_raw_items_status;
DROP TABLE IF EXISTS raw_anime_items;

-- 移除 title_parsers
DROP INDEX IF EXISTS idx_title_parsers_priority;
DROP TABLE IF EXISTS title_parsers;

-- 移除 ENUM
DROP TYPE IF EXISTS parser_source_type;
