-- 作品身分改由 fetcher 提供的 external id 決定（docs/superpowers/specs/2026-07-15-metadata-identity-design.md）

-- 季 → 外部身分。UNIQUE (namespace, external_id) 讓同一個 bgm id 不可能長出兩季
CREATE TABLE anime_external_ids (
    external_ref_id SERIAL PRIMARY KEY,
    anime_id INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
    namespace TEXT NOT NULL,
    external_id TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'fetcher' CHECK (source IN ('fetcher', 'manual')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (namespace, external_id),
    UNIQUE (anime_id, namespace)
);

-- metadata service 宣告它能解析的 namespace
CREATE TABLE metadata_namespaces (
    module_id INTEGER NOT NULL REFERENCES service_modules(module_id) ON DELETE CASCADE,
    namespace TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (module_id, namespace)
);

-- 系列身分（第一季的 external id）、名稱來源、系列簡介、soft delete
ALTER TABLE anime_works
    ADD COLUMN series_namespace TEXT NULL,
    ADD COLUMN series_external_id TEXT NULL,
    ADD COLUMN title_source TEXT NOT NULL DEFAULT 'parser'
        CHECK (title_source IN ('parser', 'bgm', 'manual')),
    ADD COLUMN description TEXT NULL,
    ADD COLUMN premiered DATE NULL,
    ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN soft_deleted_at TIMESTAMP NULL,
    ADD CONSTRAINT anime_works_series_identity_unique UNIQUE (series_namespace, series_external_id);

-- 季名（bgm 中文名 / 原名）
ALTER TABLE animes
    ADD COLUMN title TEXT NULL,
    ADD COLUMN original_title TEXT NULL;

-- 有值：季的封面；NULL：系列海報
ALTER TABLE anime_cover_images
    ADD COLUMN anime_id INTEGER NULL REFERENCES animes(anime_id) ON DELETE CASCADE;
CREATE INDEX idx_anime_cover_images_season_cover ON anime_cover_images(anime_id);

-- 讓 reparse 也能用 fetcher 回報的身分
ALTER TABLE raw_anime_items
    ADD COLUMN external_ids TEXT[] NOT NULL DEFAULT '{}';
