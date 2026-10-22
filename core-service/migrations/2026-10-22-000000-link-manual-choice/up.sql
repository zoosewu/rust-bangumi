-- 使用者手動指定的版本：不受過濾規則影響，同集的其他版本（含之後新出的）一律不採用
ALTER TABLE anime_links ADD COLUMN manual_choice BOOLEAN NOT NULL DEFAULT false;
