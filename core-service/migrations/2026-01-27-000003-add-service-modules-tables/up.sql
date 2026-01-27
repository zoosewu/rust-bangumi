-- Create downloader_modules table
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

-- Create viewer_modules table
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
