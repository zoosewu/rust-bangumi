-- ============================================================================
-- Initial Database Schema
-- ============================================================================
-- This migration creates all the core tables needed for the Bangumi application
-- Created at: 2026-01-23

-- ============================================================================
-- 1. Seasons Table
-- ============================================================================
CREATE TABLE seasons (
  season_id SERIAL PRIMARY KEY,
  year INTEGER NOT NULL,
  season VARCHAR(10) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(year, season)
);

-- ============================================================================
-- 2. Animes Table
-- ============================================================================
CREATE TABLE animes (
  anime_id SERIAL PRIMARY KEY,
  title VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 3. Anime Series Table
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
-- 4. Subtitle Groups Table
-- ============================================================================
CREATE TABLE subtitle_groups (
  group_id SERIAL PRIMARY KEY,
  group_name VARCHAR(255) NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 5. Anime Links Table
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
  UNIQUE(series_id, group_id, episode_no)
);

-- ============================================================================
-- 6. Filter Rules Table
-- ============================================================================
CREATE TABLE filter_rules (
  rule_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  rule_order INTEGER NOT NULL,
  rule_type VARCHAR(20) NOT NULL CHECK (rule_type IN ('Positive', 'Negative')),
  regex_pattern TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(series_id, group_id, rule_order)
);

-- ============================================================================
-- 7. Downloads Table
-- ============================================================================
CREATE TABLE downloads (
  download_id SERIAL PRIMARY KEY,
  link_id INTEGER NOT NULL REFERENCES anime_links(link_id) ON DELETE CASCADE,
  downloader_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'downloading', 'completed', 'failed')),
  progress REAL,
  downloaded_bytes BIGINT,
  total_bytes BIGINT,
  error_message TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 8. Cron Logs Table
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
-- 9. Fetcher Modules Table
-- ============================================================================
CREATE TABLE fetcher_modules (
  fetcher_id SERIAL PRIMARY KEY,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema JSONB,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 10. RSS Subscriptions Table
-- ============================================================================
CREATE TABLE rss_subscriptions (
  subscription_id SERIAL PRIMARY KEY,
  fetcher_id INTEGER NOT NULL REFERENCES fetcher_modules(fetcher_id) ON DELETE CASCADE,
  rss_url VARCHAR(2048) NOT NULL,
  name VARCHAR(255),
  description TEXT,
  last_fetched_at TIMESTAMP,
  next_fetch_at TIMESTAMP,
  fetch_interval_minutes INTEGER NOT NULL DEFAULT 60,
  is_active BOOLEAN NOT NULL DEFAULT true,
  config JSONB,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(fetcher_id, rss_url)
);

-- ============================================================================
-- 11. Subscription Conflicts Table
-- ============================================================================
CREATE TABLE subscription_conflicts (
  conflict_id SERIAL PRIMARY KEY,
  subscription_id INTEGER NOT NULL REFERENCES rss_subscriptions(subscription_id) ON DELETE CASCADE,
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
