-- Enable pg_trgm extension for ILIKE index support
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Add GIN index on raw_anime_items.title for efficient ILIKE search
CREATE INDEX IF NOT EXISTS idx_raw_items_title_trgm
  ON raw_anime_items USING GIN (title gin_trgm_ops);
