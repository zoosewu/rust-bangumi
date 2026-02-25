ALTER TYPE filter_target_type RENAME VALUE 'anime' TO 'anime_series';
ALTER TYPE filter_target_type RENAME VALUE 'anime_work' TO 'anime';

ALTER TABLE anime_cover_images RENAME COLUMN work_id TO anime_id;
ALTER TABLE anime_link_conflicts RENAME COLUMN anime_id TO series_id;
ALTER TABLE anime_links RENAME COLUMN anime_id TO series_id;

ALTER TABLE animes RENAME COLUMN anime_id TO series_id;
ALTER TABLE animes RENAME COLUMN work_id TO anime_id;
ALTER TABLE animes RENAME TO anime_series;

ALTER TABLE anime_works RENAME COLUMN work_id TO anime_id;
ALTER TABLE anime_works RENAME TO animes;
