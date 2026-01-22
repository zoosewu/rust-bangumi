# RSS Subscriptions Architecture Documentation

## Overview

The RSS Subscriptions system is a core component of the Bangumi application that manages the subscription and distribution of RSS feeds across multiple fetcher modules. It handles the lifecycle of subscription management, conflict resolution, and integration with fetcher services.

## System Architecture

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    Client Application                        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              HTTP API (Axum Web Framework)                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  POST   /subscriptions                               │   │
│  │  GET    /subscriptions                               │   │
│  │  GET    /fetcher-modules/{fetcher_id}/subscriptions  │   │
│  │  DELETE /subscriptions/{rss_url}                     │   │
│  │  GET    /conflicts                                   │   │
│  │  POST   /conflicts/{conflict_id}/resolve             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│         Business Logic Layer (Handlers + Services)           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  handlers/subscriptions.rs                           │   │
│  │  handlers/conflict_resolution.rs                     │   │
│  │  services/                                           │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│           Data Access Layer (Diesel ORM)                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  rss_subscriptions                                   │   │
│  │  subscription_conflicts                              │   │
│  │  fetcher_modules                                     │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│           PostgreSQL Database                                │
└─────────────────────────────────────────────────────────────┘
```

## Data Models

### 1. Fetcher Module

**Table:** `fetcher_modules`

**Purpose:** Represents registered fetcher services that can handle RSS subscriptions.

```rust
pub struct FetcherModule {
    pub fetcher_id: i32,           // Unique identifier
    pub name: String,               // Fetcher service name (e.g., "mikanani-fetcher")
    pub version: String,            // Service version (e.g., "1.0.0")
    pub description: Option<String>, // Human-readable description
    pub is_enabled: bool,           // Whether this fetcher is active
    pub config_schema: Option<String>, // JSON schema for configuration
    pub created_at: NaiveDateTime,  // Creation timestamp
    pub updated_at: NaiveDateTime,  // Last update timestamp
}
```

**Fields:**
- `fetcher_id`: Primary key, auto-generated
- `name`: Unique identifier for the fetcher service
- `version`: Semantic versioning of the fetcher
- `description`: Optional description of what this fetcher handles
- `is_enabled`: Soft disable/enable without deletion
- `config_schema`: JSON schema describing configuration options

**Example:**
```json
{
  "fetcher_id": 1,
  "name": "mikanani-fetcher",
  "version": "1.0.0",
  "description": "RSS fetcher for Mikanani anime releases",
  "is_enabled": true,
  "config_schema": "{...}",
  "created_at": "2025-01-22T10:00:00",
  "updated_at": "2025-01-22T10:00:00"
}
```

### 2. RSS Subscription

**Table:** `rss_subscriptions`

**Purpose:** Stores RSS feed subscriptions and their assignment to fetcher modules.

```rust
pub struct RssSubscription {
    pub subscription_id: i32,        // Unique identifier
    pub fetcher_id: i32,             // Reference to fetcher_modules
    pub rss_url: String,             // RSS feed URL (unique)
    pub name: Option<String>,        // Display name
    pub description: Option<String>, // Feed description
    pub last_fetched_at: Option<NaiveDateTime>, // Last successful fetch
    pub next_fetch_at: Option<NaiveDateTime>,   // Scheduled next fetch
    pub fetch_interval_minutes: i32, // Interval between fetches
    pub is_active: bool,             // Whether subscription is active
    pub config: Option<String>,      // Fetcher-specific configuration
    pub created_at: NaiveDateTime,   // Creation timestamp
    pub updated_at: NaiveDateTime,   // Last update timestamp
}
```

**Fields:**
- `subscription_id`: Primary key, auto-generated
- `fetcher_id`: Foreign key to `fetcher_modules`
- `rss_url`: Unique URL of the RSS feed (prevents duplicates)
- `name`: Optional user-friendly name
- `description`: Optional metadata about the feed
- `last_fetched_at`: Tracks successful fetch history
- `next_fetch_at`: Scheduler guidance
- `fetch_interval_minutes`: Controls fetch frequency
- `is_active`: Soft delete mechanism
- `config`: Fetcher-specific settings (JSON string)

**Example:**
```json
{
  "subscription_id": 42,
  "fetcher_id": 1,
  "rss_url": "https://mikanani.me/rss/serials",
  "name": "Mikanani Latest Releases",
  "description": "Latest anime releases from Mikanani",
  "last_fetched_at": "2025-01-22T09:30:00",
  "next_fetch_at": "2025-01-22T10:30:00",
  "fetch_interval_minutes": 60,
  "is_active": true,
  "config": "{\"source_type\": \"mikanani\"}",
  "created_at": "2025-01-20T00:00:00",
  "updated_at": "2025-01-22T09:30:00"
}
```

### 3. Subscription Conflict

**Table:** `subscription_conflicts`

**Purpose:** Handles scenarios where multiple fetchers are suitable for a single subscription.

```rust
pub struct SubscriptionConflict {
    pub conflict_id: i32,              // Unique identifier
    pub subscription_id: i32,          // Reference to rss_subscriptions
    pub conflict_type: String,         // Type of conflict (e.g., "multi_fetcher_match")
    pub conflict_data: String,         // JSON data with conflict details
    pub resolution_status: String,     // "unresolved" | "resolved"
    pub resolution_data: Option<String>, // JSON data with resolution info
    pub resolved_at: Option<NaiveDateTime>, // When conflict was resolved
    pub created_at: NaiveDateTime,     // When conflict was detected
    pub updated_at: NaiveDateTime,     // Last update timestamp
}
```

**Fields:**
- `conflict_id`: Primary key, auto-generated
- `subscription_id`: Foreign key to `rss_subscriptions`
- `conflict_type`: Categorizes the type of conflict
- `conflict_data`: JSON containing:
  - `candidate_fetcher_ids`: Array of fetcher IDs that can handle this subscription
  - `conflict_reason`: Human-readable reason for the conflict
- `resolution_status`: Current state of the conflict
- `resolution_data`: JSON containing:
  - `resolved_fetcher_id`: The selected fetcher
  - `resolved_at`: When the resolution occurred
  - `resolution_reason`: Optional explanation

**Example:**
```json
{
  "conflict_id": 15,
  "subscription_id": 42,
  "conflict_type": "multi_fetcher_match",
  "conflict_data": "{\"candidate_fetcher_ids\": [1, 3, 5], \"conflict_reason\": \"Multiple fetchers can handle anime feeds\"}",
  "resolution_status": "resolved",
  "resolution_data": "{\"resolved_fetcher_id\": 1, \"resolved_at\": \"2025-01-22T10:15:00\"}",
  "resolved_at": "2025-01-22T10:15:00",
  "created_at": "2025-01-22T10:10:00",
  "updated_at": "2025-01-22T10:15:00"
}
```

## API Endpoints

### 1. Create Subscription

**Endpoint:** `POST /subscriptions`

**Purpose:** Register a new RSS feed subscription

**Request Body:**
```json
{
  "fetcher_id": 1,
  "rss_url": "https://mikanani.me/rss/serials",
  "name": "Mikanani Latest",
  "description": "Latest anime releases",
  "fetch_interval_minutes": 60,
  "config": "{\"source\": \"mikanani\"}"
}
```

**Response (201 Created):**
```json
{
  "subscription_id": 42,
  "fetcher_id": 1,
  "rss_url": "https://mikanani.me/rss/serials",
  "name": "Mikanani Latest",
  "description": "Latest anime releases",
  "last_fetched_at": null,
  "next_fetch_at": "2025-01-22T10:00:00",
  "fetch_interval_minutes": 60,
  "is_active": true,
  "config": "{\"source\": \"mikanani\"}",
  "created_at": "2025-01-22T10:00:00",
  "updated_at": "2025-01-22T10:00:00"
}
```

**Error Responses:**
- `409 Conflict`: Subscription with same RSS URL already exists
  ```json
  {
    "error": "duplicate_url",
    "message": "Subscription already exists for this URL: ..."
  }
  ```
- `500 Internal Server Error`: Database connection or write error
  ```json
  {
    "error": "database_error",
    "message": "Failed to create subscription: ..."
  }
  ```

**Workflow:**
1. Validate request payload
2. Check for existing subscription with same RSS URL
3. If duplicate found, return 409 Conflict
4. Insert new subscription into database
5. Broadcast subscription event to all registered fetchers
6. Return created subscription with 201 status

### 2. List All Subscriptions

**Endpoint:** `GET /subscriptions`

**Purpose:** Retrieve all active subscriptions

**Response (200 OK):**
```json
{
  "subscriptions": [
    {
      "subscription_id": 42,
      "fetcher_id": 1,
      "rss_url": "https://mikanani.me/rss/serials",
      "name": "Mikanani Latest",
      "fetch_interval_minutes": 60,
      "is_active": true,
      ...
    },
    {...}
  ]
}
```

**Error Responses:**
- `500 Internal Server Error`: Database error
  ```json
  {
    "error": "database_error",
    "message": "Failed to list subscriptions: ...",
    "subscriptions": []
  }
  ```

**Filtering:**
- Only returns subscriptions with `is_active = true`

### 3. List Subscriptions by Fetcher

**Endpoint:** `GET /fetcher-modules/{fetcher_id}/subscriptions`

**Purpose:** Retrieve all active subscriptions for a specific fetcher

**Parameters:**
- `fetcher_id` (path): ID of the fetcher module

**Response (200 OK):**
```json
{
  "fetcher_id": 1,
  "urls": [
    "https://mikanani.me/rss/serials",
    "https://example.com/rss/anime.xml"
  ]
}
```

**Usage:** Fetcher services call this endpoint periodically to obtain their assigned subscriptions

### 4. List Fetcher Modules

**Endpoint:** `GET /fetcher-modules`

**Purpose:** Retrieve all registered fetcher modules

**Response (200 OK):**
```json
{
  "fetcher_modules": [
    {
      "fetcher_id": 1,
      "name": "mikanani-fetcher",
      "version": "1.0.0",
      "description": "Mikanani RSS fetcher",
      "is_enabled": true,
      "config_schema": null,
      "created_at": "2025-01-20T00:00:00",
      "updated_at": "2025-01-20T00:00:00"
    },
    {...}
  ]
}
```

### 5. Delete Subscription

**Endpoint:** `DELETE /subscriptions/{rss_url}`

**Purpose:** Deactivate a subscription (soft delete)

**Parameters:**
- `rss_url` (path): RSS URL to delete

**Response (200 OK):**
```json
{
  "message": "Subscription deleted successfully",
  "rss_url": "https://mikanani.me/rss/serials",
  "rows_deleted": 1
}
```

**Error Responses:**
- `404 Not Found`: Subscription with given URL not found
  ```json
  {
    "error": "not_found",
    "message": "Subscription not found for URL: ..."
  }
  ```

### 6. Get Pending Conflicts

**Endpoint:** `GET /conflicts`

**Purpose:** Retrieve all unresolved subscription conflicts

**Response (200 OK):**
```json
{
  "conflicts": [
    {
      "conflict_id": 15,
      "subscription_id": 42,
      "rss_url": "https://mikanani.me/rss/serials",
      "conflict_type": "multi_fetcher_match",
      "conflict_data": {
        "candidate_fetcher_ids": [1, 3, 5],
        "conflict_reason": "Multiple fetchers can handle anime feeds"
      },
      "candidate_fetchers": [
        {"fetcher_id": 1, "name": "mikanani-fetcher"},
        {"fetcher_id": 3, "name": "bangumi-fetcher"},
        {"fetcher_id": 5, "name": "generic-fetcher"}
      ],
      "created_at": "2025-01-22T10:10:00"
    }
  ]
}
```

**Filtering:**
- Only returns conflicts with `resolution_status = "unresolved"`
- Includes fetcher metadata for each candidate

### 7. Resolve Conflict

**Endpoint:** `POST /conflicts/{conflict_id}/resolve`

**Purpose:** Resolve a conflict by assigning subscription to specific fetcher

**Parameters:**
- `conflict_id` (path): ID of the conflict to resolve

**Request Body:**
```json
{
  "fetcher_id": 1
}
```

**Response (200 OK):**
```json
{
  "message": "Conflict resolved successfully",
  "conflict_id": 15,
  "subscription_id": 42,
  "resolved_fetcher_id": 1,
  "resolved_at": "2025-01-22T10:15:00"
}
```

**Error Responses:**
- `404 Not Found`: Conflict not found
  ```json
  {
    "error": "not_found",
    "message": "Conflict not found: ..."
  }
  ```
- `400 Bad Request`: Fetcher not a valid candidate
  ```json
  {
    "error": "invalid_fetcher",
    "message": "Fetcher X is not a candidate for this conflict",
    "candidates": [1, 3, 5]
  }
  ```

**Workflow:**
1. Verify conflict exists
2. Parse candidate fetchers from conflict data
3. Validate selected fetcher is in candidate list
4. Verify selected fetcher exists and is enabled
5. Update `rss_subscriptions` with new `fetcher_id`
6. Update `subscription_conflicts` with resolution status and data
7. Return success response

## Workflows

### Workflow 1: New Subscription Registration

```
User Request
    │
    ├─► POST /subscriptions
    │
    ├─► Validate request payload
    │
    ├─► Check for duplicate RSS URL
    │   ├─► Found: Return 409 Conflict
    │   └─► Not found: Continue
    │
    ├─► Create RssSubscription record
    │   ├─► Assign default or specified fetcher
    │   ├─► Set fetch_interval_minutes
    │   ├─► Set is_active = true
    │   └─► Set next_fetch_at = now
    │
    ├─► Broadcast subscription event
    │   └─► Notify all registered fetchers of new subscription
    │
    └─► Return 201 Created with subscription details
```

### Workflow 2: Fetcher Retrieves Subscriptions

```
Fetcher Service (Periodic)
    │
    └─► GET /fetcher-modules/{fetcher_id}/subscriptions
        │
        ├─► Query rss_subscriptions where fetcher_id matches
        │
        ├─► Filter for is_active = true
        │
        ├─► Extract RSS URLs
        │
        └─► Return list of URLs for fetcher to process
```

### Workflow 3: Conflict Detection and Resolution

```
Conflict Detection (External Service)
    │
    └─► Detects multiple suitable fetchers for subscription
        │
        ├─► Create SubscriptionConflict record
        │   ├─► Set conflict_type (e.g., "multi_fetcher_match")
        │   ├─► Set conflict_data with candidate_fetcher_ids
        │   └─► Set resolution_status = "unresolved"
        │
        └─► Human/Admin Action
            │
            ├─► GET /conflicts
            │   └─► Review pending conflicts
            │
            └─► POST /conflicts/{conflict_id}/resolve
                │
                ├─► Validate fetcher selection
                │
                ├─► Update subscription fetcher_id
                │
                ├─► Mark conflict as resolved
                │
                └─► Return resolution confirmation
```

### Workflow 4: Subscription Cleanup

```
Admin Action
    │
    └─► DELETE /subscriptions/{rss_url}
        │
        ├─► Find subscription by RSS URL
        │
        ├─► Mark as inactive (is_active = false)
        │   OR
        ├─► Hard delete subscription record
        │
        ├─► Clean up associated conflicts
        │
        └─► Return deletion confirmation
```

## Database Relationships

```
┌─────────────────────────────────────────┐
│         fetcher_modules                 │
│  ┌─────────────────────────────────┐   │
│  │ PK: fetcher_id                  │   │
│  │     name (UNIQUE)               │   │
│  │     version                     │   │
│  │     is_enabled                  │   │
│  │     ...                         │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
           ▲        ▲
           │        │
           │ 1:N    │ 1:N
           │        │
           │        │
┌──────────┴─┐    ┌─┴──────────┐
│rss_subscr. │    │subscr.conf.│
└──────────┬─┘    └─┬──────────┘
           │        │
           └────┬───┘
       1:1 (via subscription_id)
```

**Relationships:**
- One `FetcherModule` can have many `RssSubscription`s (1:N via `fetcher_id`)
- One `RssSubscription` can have at most one active `SubscriptionConflict` (1:1 via `subscription_id`)
- One `RssSubscription` can be associated with multiple `SubscriptionConflict`s historically (1:N via `subscription_id`)

## Key Design Decisions

### 1. Unique RSS URLs
- Each RSS URL can only be subscribed to once
- Enforced at application layer (returns 409 Conflict on duplicate)
- Could be enforced at database layer with UNIQUE constraint

### 2. Soft Deletes vs Hard Deletes
- Current implementation supports both approaches
- `is_active` field allows soft delete without data loss
- Hard delete removes all records and associated conflicts

### 3. Conflict Resolution
- Conflicts are designed to be resolved by human decision
- Multiple candidates indicate uncertainty in automatic assignment
- Resolution is idempotent - resolving same conflict twice has no side effects

### 4. Broadcast Mechanism
- New subscriptions are broadcast to all fetchers
- Fetchers can subscribe to subscription events
- Decouples subscription registration from fetcher processing

## State Transitions

### Subscription States
```
NOT_ACTIVE ─────────────┐
   ▲                    │
   │                    ▼
   │              ACTIVE (is_active=true)
   │                    │
   └────────────────────┘
      (soft or hard delete)
```

### Conflict States
```
UNRESOLVED ──────────────► RESOLVED
  │                           │
  └───────────────────────────┘
     (can remain unresolved indefinitely)
```

## Error Handling

### Application-Level Errors

| Error Code | Scenario | Response |
|-----------|----------|----------|
| 400 | Invalid fetcher in conflict resolution | "invalid_fetcher" |
| 404 | Resource not found (conflict, subscription) | "not_found" |
| 409 | Duplicate RSS URL | "duplicate_url" |
| 500 | Database connection/query error | "database_error" or "connection_error" |

### Logging
All operations are logged with structured tracing:
- `tracing::info!()` for successful operations
- `tracing::warn!()` for recoverable issues
- `tracing::error!()` for failures

## Integration Points

### 1. Fetcher Services
- Poll `/fetcher-modules/{fetcher_id}/subscriptions` regularly
- Receive new subscriptions via broadcast
- Report fetch results to `/fetcher-results` endpoint

### 2. Conflict Detection
- External service detects conflicts and POSTs to system
- System stores conflicts in database
- Admin/operator reviews via `/conflicts` endpoint

### 3. Event Broadcasting
- New subscriptions trigger broadcast to registered fetchers
- Uses `tokio::sync::broadcast` channel
- Non-blocking for subscription creation

## Performance Considerations

### Query Optimization
- Subscriptions filtered by `is_active` to reduce result sets
- Fetcher queries by `fetcher_id` indexed
- Conflict queries by `resolution_status` indexed

### Database Indexes
Recommended indexes:
```sql
CREATE INDEX idx_rss_subscriptions_fetcher_id
    ON rss_subscriptions(fetcher_id);

CREATE INDEX idx_rss_subscriptions_is_active
    ON rss_subscriptions(is_active);

CREATE INDEX idx_subscriptions_conflicts_status
    ON subscription_conflicts(resolution_status);
```

### Connection Pooling
- Uses `diesel::r2d2` connection pool
- Default pool size: 5 connections
- Configurable via environment variables

## Troubleshooting

### Common Issues

**Issue:** New subscriptions not appearing in fetcher subscription list
- Check `is_active = true` in database
- Verify correct `fetcher_id` is assigned
- Check database connection pool status

**Issue:** Conflicts remain unresolved indefinitely
- Check `resolution_status = "unresolved"` in database
- Verify conflict data contains valid `candidate_fetcher_ids`
- Review logs for resolution attempt failures

**Issue:** Duplicate RSS URL error on valid new URL
- Check for existing subscriptions with similar URLs (whitespace, encoding)
- Verify previous subscription was properly deleted
- Check for case-sensitivity issues

**Issue:** Broadcast not reaching fetcher services
- Verify fetcher is subscribed to broadcast channel
- Check network connectivity between services
- Review application logs for broadcast errors

## Future Enhancements

### 1. Automatic Conflict Resolution
- Machine learning-based fetcher selection
- Historical success rate tracking
- Automatic assignment based on metrics

### 2. Subscription Analytics
- Track fetch success/failure rates
- Monitor subscription usage patterns
- Identify problematic feeds

### 3. Advanced Scheduling
- Dynamic fetch interval adjustment
- Priority-based scheduling
- Batch optimization

### 4. Multi-Source Subscriptions
- Support for HTTP feeds with custom headers
- Authentication token management
- Rate limiting per source

### 5. Subscription Grouping
- Organize subscriptions into collections
- Shared configuration groups
- Batch operations

### 6. Detailed Conflict Metadata
- Store conflict resolution history
- Track human decisions for ML training
- Provide decision audit trail

## Database Migration

To set up the database schema:

```bash
cd core-service
diesel migration run
```

This will create the required tables:
- `fetcher_modules`
- `rss_subscriptions`
- `subscription_conflicts`

Ensure PostgreSQL is running and DATABASE_URL is properly configured.

## References

- [Axum Web Framework Documentation](https://docs.rs/axum/latest/axum/)
- [Diesel ORM Documentation](https://diesel.rs/)
- [Tokio Async Runtime](https://tokio.rs/)
- [Serde Serialization](https://serde.rs/)
