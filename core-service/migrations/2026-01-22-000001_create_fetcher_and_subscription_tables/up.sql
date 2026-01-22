-- Create fetcher_modules table for fetcher registration
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

-- Create rss_subscriptions table for URL management
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

-- Create subscription_conflicts table for conflict resolution history
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
