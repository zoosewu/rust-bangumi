-- Add base_url column to fetcher_modules for storing Fetcher service URLs
ALTER TABLE fetcher_modules
ADD COLUMN base_url VARCHAR(255) NOT NULL DEFAULT 'http://localhost:3000';

-- Index for faster lookups by base_url
CREATE INDEX idx_fetcher_modules_base_url ON fetcher_modules(base_url);
