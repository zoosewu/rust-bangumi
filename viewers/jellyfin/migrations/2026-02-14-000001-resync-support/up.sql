ALTER TABLE sync_tasks ADD COLUMN anime_title TEXT;
ALTER TABLE sync_tasks ADD COLUMN series_no INT;
ALTER TABLE sync_tasks ADD COLUMN subtitle_group TEXT;
ALTER TABLE sync_tasks ADD COLUMN task_type VARCHAR(10) NOT NULL DEFAULT 'sync';
