-- ===================================================
-- Step 1: animes → anime_works，PK: anime_id → work_id
-- ===================================================
ALTER TABLE animes RENAME TO anime_works;
ALTER TABLE anime_works RENAME COLUMN anime_id TO work_id;

-- ===================================================
-- Step 2: anime_series → animes
--   先改 FK 欄位名（anime_id → work_id），再改 PK（series_id → anime_id）
--   必須先改 FK 欄位，否則兩個欄位都叫 anime_id 會衝突
-- ===================================================
ALTER TABLE anime_series RENAME TO animes;
ALTER TABLE animes RENAME COLUMN anime_id TO work_id;
ALTER TABLE animes RENAME COLUMN series_id TO anime_id;

-- ===================================================
-- Step 3: 其他表的 FK 欄位重命名
-- ===================================================
ALTER TABLE anime_links RENAME COLUMN series_id TO anime_id;
ALTER TABLE anime_link_conflicts RENAME COLUMN series_id TO anime_id;
ALTER TABLE anime_cover_images RENAME COLUMN anime_id TO work_id;

-- ===================================================
-- Step 4: filter_target_type enum 值更新
-- 先改 anime → anime_work，再把 anime_series → anime
-- ===================================================
ALTER TYPE filter_target_type RENAME VALUE 'anime' TO 'anime_work';
ALTER TYPE filter_target_type RENAME VALUE 'anime_series' TO 'anime';
