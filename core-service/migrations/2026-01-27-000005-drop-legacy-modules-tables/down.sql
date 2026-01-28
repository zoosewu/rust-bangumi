-- Recreate fetcher_modules table
CREATE TABLE fetcher_modules (
  fetcher_id SERIAL PRIMARY KEY,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL
);

CREATE INDEX idx_fetcher_modules_base_url ON fetcher_modules(base_url);

-- Recreate downloader_modules table
CREATE TABLE downloader_modules (
  downloader_id SERIAL PRIMARY KEY,
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

CREATE INDEX idx_downloader_modules_base_url ON downloader_modules(base_url);

-- Recreate viewer_modules table
CREATE TABLE viewer_modules (
  viewer_id SERIAL PRIMARY KEY,
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

CREATE INDEX idx_viewer_modules_base_url ON viewer_modules(base_url);

-- Restore data from service_modules
INSERT INTO fetcher_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'fetcher'::module_type;

INSERT INTO downloader_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'downloader'::module_type;

INSERT INTO viewer_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'viewer'::module_type;

-- Restore foreign key constraint
ALTER TABLE subscriptions
ADD CONSTRAINT rss_subscriptions_fetcher_id_fkey
FOREIGN KEY (fetcher_id) REFERENCES fetcher_modules(fetcher_id);
