-- bangumi.tv metadata cache
CREATE TABLE bangumi_subjects (
    bangumi_id      INT PRIMARY KEY,
    title           TEXT NOT NULL,
    title_cn        TEXT,
    summary         TEXT,
    rating          REAL,
    cover_url       TEXT,
    air_date        DATE,
    episode_count   INT,
    raw_json        JSONB,
    fetched_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Single episode metadata cache
CREATE TABLE bangumi_episodes (
    bangumi_ep_id   INT PRIMARY KEY,
    bangumi_id      INT NOT NULL REFERENCES bangumi_subjects(bangumi_id) ON DELETE CASCADE,
    episode_no      INT NOT NULL,
    title           TEXT,
    title_cn        TEXT,
    air_date        DATE,
    summary         TEXT,
    fetched_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Core series_id -> bangumi.tv subject_id mapping
CREATE TABLE bangumi_mapping (
    core_series_id  INT PRIMARY KEY,
    bangumi_id      INT NOT NULL REFERENCES bangumi_subjects(bangumi_id),
    title_cache     TEXT,
    source          VARCHAR(20) NOT NULL DEFAULT 'auto_search',
    created_at      TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Sync task history
CREATE TABLE sync_tasks (
    task_id         SERIAL PRIMARY KEY,
    download_id     INT NOT NULL,
    core_series_id  INT NOT NULL,
    episode_no      INT NOT NULL,
    source_path     TEXT NOT NULL,
    target_path     TEXT,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    created_at      TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMP
);

CREATE INDEX idx_sync_tasks_status ON sync_tasks(status);
CREATE INDEX idx_sync_tasks_download_id ON sync_tasks(download_id);
