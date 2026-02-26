ALTER TABLE downloads
    DROP COLUMN IF EXISTS video_file,
    DROP COLUMN IF EXISTS subtitle_files;
