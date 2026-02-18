-- ============================================================================
-- Consolidated Initial Schema - Jellyfin Viewer
-- ============================================================================
-- Consolidated from 3 incremental migrations on 2026-02-17.

-- ============================================================================
-- 1. Bangumi.tv Metadata Cache
-- ============================================================================
CREATE TABLE bangumi_subjects (
    bangumi_id      INT PRIMARY KEY,
    title           TEXT NOT NULL,
    title_cn        TEXT,
    summary         TEXT,
    rating          REAL,
    cover_url       TEXT,
    air_date        DATE
);

-- ============================================================================
-- 2. Episode Metadata Cache
-- ============================================================================
CREATE TABLE bangumi_episodes (
    bangumi_ep_id   INT PRIMARY KEY,
    bangumi_id      INT NOT NULL REFERENCES bangumi_subjects(bangumi_id) ON DELETE CASCADE,
    episode_no      INT NOT NULL,
    title           TEXT,
    title_cn        TEXT,
    air_date        DATE,
    summary         TEXT
);

-- ============================================================================
-- 3. Core Series → Bangumi.tv Mapping
-- ============================================================================
CREATE TABLE bangumi_mapping (
    core_series_id  INT PRIMARY KEY,
    bangumi_id      INT NOT NULL REFERENCES bangumi_subjects(bangumi_id)
);

-- ============================================================================
-- 4. Sync Tasks
-- ============================================================================
CREATE TABLE sync_tasks (
    task_id         SERIAL PRIMARY KEY,
    download_id     INT NOT NULL,
    target_path     TEXT,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
    completed_at    TIMESTAMP
);

CREATE INDEX idx_sync_tasks_status ON sync_tasks(status);
CREATE INDEX idx_sync_tasks_download_id ON sync_tasks(download_id);
