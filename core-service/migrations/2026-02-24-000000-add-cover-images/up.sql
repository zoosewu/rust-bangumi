-- ============================================================================
-- Add 'metadata' value to module_type enum
-- ============================================================================
ALTER TYPE module_type ADD VALUE IF NOT EXISTS 'metadata';

-- ============================================================================
-- Create anime_cover_images table
-- ============================================================================
CREATE TABLE anime_cover_images (
    cover_id          SERIAL PRIMARY KEY,
    anime_id          INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
    image_url         TEXT NOT NULL,
    service_module_id INTEGER REFERENCES service_modules(module_id) ON DELETE SET NULL,
    source_name       VARCHAR(100) NOT NULL,
    is_default        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(anime_id, image_url)
);

CREATE INDEX idx_anime_cover_images_anime_id ON anime_cover_images(anime_id);
CREATE INDEX idx_anime_cover_images_default ON anime_cover_images(anime_id) WHERE is_default = TRUE;
