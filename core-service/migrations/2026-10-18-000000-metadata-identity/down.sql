ALTER TABLE raw_anime_items DROP COLUMN external_ids;

DROP INDEX idx_anime_cover_images_season_cover;
ALTER TABLE anime_cover_images DROP COLUMN anime_id;

ALTER TABLE animes
    DROP COLUMN original_title,
    DROP COLUMN title;

ALTER TABLE anime_works
    DROP CONSTRAINT anime_works_series_identity_unique,
    DROP COLUMN soft_deleted_at,
    DROP COLUMN is_active,
    DROP COLUMN premiered,
    DROP COLUMN description,
    DROP COLUMN title_source,
    DROP COLUMN series_external_id,
    DROP COLUMN series_namespace;

DROP TABLE metadata_namespaces;
DROP TABLE anime_external_ids;
