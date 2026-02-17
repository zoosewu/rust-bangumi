-- ============================================================================
-- Consolidated Initial Schema
-- ============================================================================
-- All tables for the Bangumi core-service database.
-- Consolidated from 17 incremental migrations on 2026-02-17.

-- ============================================================================
-- Custom Types
-- ============================================================================
CREATE TYPE module_type AS ENUM ('fetcher', 'downloader', 'viewer');
CREATE TYPE parser_source_type AS ENUM ('regex', 'static');
CREATE TYPE filter_target_type AS ENUM ('global', 'anime', 'subtitle_group', 'anime_series', 'fetcher', 'subscription');

-- ============================================================================
-- 1. Seasons
-- ============================================================================
CREATE TABLE seasons (
  season_id SERIAL PRIMARY KEY,
  year INTEGER NOT NULL,
  season VARCHAR(10) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(year, season)
);

-- ============================================================================
-- 2. Animes
-- ============================================================================
CREATE TABLE animes (
  anime_id SERIAL PRIMARY KEY,
  title VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 3. Anime Series
-- ============================================================================
CREATE TABLE anime_series (
  series_id SERIAL PRIMARY KEY,
  anime_id INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
  series_no INTEGER NOT NULL,
  season_id INTEGER NOT NULL REFERENCES seasons(season_id) ON DELETE CASCADE,
  description TEXT,
  aired_date DATE,
  end_date DATE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(anime_id, series_no)
);

-- ============================================================================
-- 4. Subtitle Groups
-- ============================================================================
CREATE TABLE subtitle_groups (
  group_id SERIAL PRIMARY KEY,
  group_name VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 5. Service Modules (consolidated fetcher/downloader/viewer)
-- ============================================================================
CREATE TABLE service_modules (
  module_id SERIAL PRIMARY KEY,
  module_type module_type NOT NULL,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_service_modules_module_type ON service_modules(module_type);
CREATE INDEX idx_service_modules_base_url ON service_modules(base_url);
CREATE INDEX idx_service_modules_name_type ON service_modules(name, module_type);

-- ============================================================================
-- 6. Subscriptions
-- ============================================================================
CREATE TABLE subscriptions (
  subscription_id SERIAL PRIMARY KEY,
  fetcher_id INTEGER NOT NULL,
  source_url VARCHAR(2048) NOT NULL,
  name VARCHAR(255),
  description TEXT,
  last_fetched_at TIMESTAMP,
  next_fetch_at TIMESTAMP,
  fetch_interval_minutes INTEGER NOT NULL DEFAULT 60,
  is_active BOOLEAN NOT NULL DEFAULT true,
  config JSONB,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  source_type VARCHAR(50) NOT NULL DEFAULT 'rss',
  assignment_status VARCHAR(20) NOT NULL DEFAULT 'pending',
  assigned_at TIMESTAMP,
  auto_selected BOOLEAN NOT NULL DEFAULT false,
  UNIQUE(fetcher_id, source_url)
);

CREATE INDEX idx_subscriptions_assignment_status ON subscriptions(assignment_status);
CREATE INDEX idx_subscriptions_source_type ON subscriptions(source_type);
CREATE INDEX idx_subscriptions_auto_selected ON subscriptions(auto_selected);
CREATE INDEX idx_subscriptions_created_at ON subscriptions(created_at);

-- ============================================================================
-- 7. Subscription Conflicts
-- ============================================================================
CREATE TABLE subscription_conflicts (
  conflict_id SERIAL PRIMARY KEY,
  subscription_id INTEGER NOT NULL REFERENCES subscriptions(subscription_id) ON DELETE CASCADE,
  conflict_type VARCHAR(50) NOT NULL,
  affected_item_id VARCHAR(255),
  conflict_data JSONB NOT NULL,
  resolution_status VARCHAR(50) NOT NULL DEFAULT 'unresolved',
  resolution_data JSONB,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  resolved_at TIMESTAMP
);

CREATE INDEX idx_subscription_conflicts_subscription_id ON subscription_conflicts(subscription_id);
CREATE INDEX idx_subscription_conflicts_resolution_status ON subscription_conflicts(resolution_status);

-- ============================================================================
-- 8. Title Parsers
-- ============================================================================
CREATE TABLE title_parsers (
    parser_id               SERIAL PRIMARY KEY,
    name                    VARCHAR(100) NOT NULL,
    description             TEXT,
    priority                INT NOT NULL DEFAULT 0,
    is_enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    condition_regex         TEXT NOT NULL,
    parse_regex             TEXT NOT NULL,
    anime_title_source      parser_source_type NOT NULL,
    anime_title_value       VARCHAR(255) NOT NULL,
    episode_no_source       parser_source_type NOT NULL,
    episode_no_value        VARCHAR(50) NOT NULL,
    series_no_source        parser_source_type,
    series_no_value         VARCHAR(50),
    subtitle_group_source   parser_source_type,
    subtitle_group_value    VARCHAR(255),
    resolution_source       parser_source_type,
    resolution_value        VARCHAR(50),
    season_source           parser_source_type,
    season_value            VARCHAR(20),
    year_source             parser_source_type,
    year_value              VARCHAR(10),
    created_at              TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP NOT NULL DEFAULT NOW(),
    created_from_type       filter_target_type DEFAULT NULL,
    created_from_id         INTEGER DEFAULT NULL
);

CREATE INDEX idx_title_parsers_priority
ON title_parsers(priority DESC)
WHERE is_enabled = TRUE;

-- ============================================================================
-- 9. Raw Anime Items
-- ============================================================================
CREATE TABLE raw_anime_items (
    item_id             SERIAL PRIMARY KEY,
    title               TEXT NOT NULL,
    description         TEXT,
    download_url        VARCHAR(2048) NOT NULL,
    pub_date            TIMESTAMP,
    subscription_id     INT NOT NULL REFERENCES subscriptions(subscription_id) ON DELETE CASCADE,
    status              VARCHAR(20) NOT NULL DEFAULT 'pending',
    parser_id           INT REFERENCES title_parsers(parser_id),
    error_message       TEXT,
    parsed_at           TIMESTAMP,
    created_at          TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(download_url)
);

CREATE INDEX idx_raw_items_status ON raw_anime_items(status);
CREATE INDEX idx_raw_items_subscription ON raw_anime_items(subscription_id);
CREATE INDEX idx_raw_items_created ON raw_anime_items(created_at DESC);

-- ============================================================================
-- 10. Anime Links
-- ============================================================================
CREATE TABLE anime_links (
  link_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  episode_no INTEGER NOT NULL,
  title VARCHAR(255),
  url TEXT NOT NULL,
  source_hash VARCHAR(255) NOT NULL,
  filtered_flag BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  raw_item_id INT REFERENCES raw_anime_items(item_id) ON DELETE SET NULL,
  download_type VARCHAR(20),
  conflict_flag BOOLEAN NOT NULL DEFAULT FALSE,
  link_status VARCHAR(20) NOT NULL DEFAULT 'active'
    CHECK (link_status IN ('active', 'resolved')),
  CONSTRAINT anime_links_source_hash_unique UNIQUE (source_hash)
);

CREATE INDEX idx_anime_links_raw_item ON anime_links(raw_item_id);
CREATE INDEX idx_anime_links_download_type ON anime_links(download_type);
CREATE INDEX idx_anime_links_conflict ON anime_links(series_id, group_id, episode_no)
  WHERE link_status = 'active';

-- ============================================================================
-- 11. Anime Link Conflicts
-- ============================================================================
CREATE TABLE anime_link_conflicts (
  conflict_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  episode_no INTEGER NOT NULL,
  resolution_status VARCHAR(20) NOT NULL DEFAULT 'unresolved'
    CHECK (resolution_status IN ('unresolved', 'resolved')),
  chosen_link_id INTEGER REFERENCES anime_links(link_id) ON DELETE SET NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMP,
  UNIQUE (series_id, group_id, episode_no)
);

CREATE INDEX idx_anime_link_conflicts_unresolved ON anime_link_conflicts(resolution_status)
  WHERE resolution_status = 'unresolved';

-- ============================================================================
-- 12. Filter Rules
-- ============================================================================
CREATE TABLE filter_rules (
  rule_id SERIAL PRIMARY KEY,
  rule_order INTEGER NOT NULL,
  regex_pattern TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  is_positive BOOLEAN NOT NULL,
  target_type filter_target_type NOT NULL,
  target_id INT,
  CONSTRAINT filter_rules_target_order_key UNIQUE (target_type, target_id, rule_order)
);

CREATE INDEX idx_filter_rules_target ON filter_rules(target_type, target_id);

-- ============================================================================
-- 13. Downloads
-- ============================================================================
CREATE TABLE downloads (
  download_id SERIAL PRIMARY KEY,
  link_id INTEGER NOT NULL REFERENCES anime_links(link_id) ON DELETE CASCADE,
  downloader_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled', 'downloader_error', 'no_downloader', 'syncing', 'synced', 'sync_failed')),
  progress REAL,
  downloaded_bytes BIGINT,
  total_bytes BIGINT,
  error_message TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  module_id INT REFERENCES service_modules(module_id),
  torrent_hash VARCHAR(255),
  file_path TEXT,
  sync_retry_count INT NOT NULL DEFAULT 0
);

CREATE INDEX idx_downloads_module_status ON downloads(module_id, status);
CREATE INDEX idx_downloads_torrent_hash ON downloads(torrent_hash);

-- ============================================================================
-- 14. Downloader Capabilities
-- ============================================================================
CREATE TABLE downloader_capabilities (
    module_id INT REFERENCES service_modules(module_id) ON DELETE CASCADE,
    download_type VARCHAR(20) NOT NULL,
    PRIMARY KEY (module_id, download_type)
);

-- ============================================================================
-- 15. Cron Logs
-- ============================================================================
CREATE TABLE cron_logs (
  log_id SERIAL PRIMARY KEY,
  fetcher_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL CHECK (status IN ('success', 'failed')),
  error_message TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 1,
  executed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cron_logs_fetcher_type ON cron_logs(fetcher_type);

-- ============================================================================
-- Default Data: Title Parsers
-- ============================================================================
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value,
    resolution_source, resolution_value
) VALUES (
    'LoliHouse 標準格式',
    '匹配 [字幕組] 動畫名稱 - 集數 [解析度] 格式',
    100,
    '^\[.+\].+\s-\s\d+',
    '^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)',
    'regex', '2',
    'regex', '3',
    NULL, NULL,
    'regex', '1',
    'regex', '4'
), (
    '六四位元 星號格式',
    '匹配以星號分隔的格式',
    90,
    '^[^★]+★.+★\d+★',
    '^([^★]+)★(.+?)★(\d+)★(\d+x\d+)',
    'regex', '2',
    'regex', '3',
    'static', '1',
    'regex', '1',
    'regex', '4'
), (
    '預設解析器',
    '嘗試匹配任何包含 - 數字 的標題',
    1,
    '.+\s-\s\d+',
    '^(.+?)\s+-\s*(\d+)',
    'regex', '1',
    'regex', '2',
    'static', '1',
    'static', '未知字幕組',
    NULL, NULL
);

INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value
) VALUES (
    'Catch-All 全匹配',
    '最低優先級，將整個標題作為動畫名稱，集數預設為 1。確保所有標題都能被解析。',
    0,
    '.+',
    '^(.+)$',
    'regex', '1',
    'static', '1',
    'static', '1',
    'static', '未知字幕組'
);
