-- ============================================================================
-- Subscription System Redesign Migration
-- ============================================================================
-- This migration updates the subscription management system to support:
-- - Fetcher priority-based selection
-- - Multi-source support (rss_url renamed to source_url with source_type)
-- - Assignment tracking and automatic selection
-- Created at: 2026-01-26

-- ============================================================================
-- 1. Add priority column to fetcher_modules
-- ============================================================================
ALTER TABLE fetcher_modules
ADD COLUMN priority INTEGER NOT NULL DEFAULT 50;

-- ============================================================================
-- 2. Rename rss_subscriptions table to subscriptions
-- ============================================================================
ALTER TABLE rss_subscriptions
RENAME TO subscriptions;

-- ============================================================================
-- 3. Rename rss_url column to source_url
-- ============================================================================
ALTER TABLE subscriptions
RENAME COLUMN rss_url TO source_url;

-- ============================================================================
-- 4. Add source_type column to subscriptions
-- ============================================================================
ALTER TABLE subscriptions
ADD COLUMN source_type VARCHAR(50) NOT NULL DEFAULT 'rss';

-- ============================================================================
-- 5. Add assignment tracking columns to subscriptions
-- ============================================================================
ALTER TABLE subscriptions
ADD COLUMN assignment_status VARCHAR(20) NOT NULL DEFAULT 'pending',
ADD COLUMN assigned_at TIMESTAMP,
ADD COLUMN auto_selected BOOLEAN NOT NULL DEFAULT false;

-- ============================================================================
-- 6. Update unique constraints
-- ============================================================================
-- Drop old constraint
ALTER TABLE subscriptions
DROP CONSTRAINT IF EXISTS rss_subscriptions_fetcher_id_rss_url_key;

-- Create new constraint with source_url
ALTER TABLE subscriptions
ADD CONSTRAINT subscriptions_fetcher_id_source_url_key UNIQUE (fetcher_id, source_url);

-- Create index for assignment_status
CREATE INDEX idx_subscriptions_assignment_status ON subscriptions(assignment_status);

-- Create index for source_type
CREATE INDEX idx_subscriptions_source_type ON subscriptions(source_type);

-- Create index for auto_selected
CREATE INDEX idx_subscriptions_auto_selected ON subscriptions(auto_selected);

-- ============================================================================
-- 7. Update subscription_conflicts table references
-- ============================================================================
-- Drop old foreign key constraint
ALTER TABLE subscription_conflicts
DROP CONSTRAINT IF EXISTS subscription_conflicts_subscription_id_fkey;

-- Add new foreign key constraint
ALTER TABLE subscription_conflicts
ADD CONSTRAINT subscription_conflicts_subscription_id_fkey
FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id) ON DELETE CASCADE;

-- Create index for faster lookups
CREATE INDEX idx_subscriptions_created_at ON subscriptions(created_at);

-- ============================================================================
-- 8. Change JSONB to TEXT for compatibility
-- ============================================================================
-- This allows easier Diesel integration without needing special JSONB support
-- ALTER TABLE fetcher_modules ALTER COLUMN config_schema TYPE TEXT;
-- ALTER TABLE subscriptions ALTER COLUMN config TYPE TEXT;
-- ALTER TABLE subscription_conflicts ALTER COLUMN conflict_data TYPE TEXT;
-- ALTER TABLE subscription_conflicts ALTER COLUMN resolution_data TYPE TEXT;
