# Mikanani RSS Fetcher Service

Microservice for parsing Mikanani anime RSS feeds and extracting anime metadata.

## Features

- **RSS Parsing**: Extract anime data from Mikanani RSS feeds using the feed-rs library
- **Retry Logic**: Exponential backoff for transient failures (maximum 3 attempts)
- **Async Fetch**: Non-blocking fetch with background task execution
- **Error Handling**: Comprehensive error logging and recovery
- **Service Registration**: Automatic registration with core service
- **Health Checks**: Built-in health check endpoint
- **URL Ownership**: Can-handle-subscription endpoint for URL routing

## API Endpoints

### POST /fetch
Trigger RSS feed fetch (async). Returns immediately with 202 Accepted, then fetches in background and calls back to core service.

**Request:**
```json
{
  "subscription_id": 123,
  "rss_url": "https://mikanani.me/RSS/Bangumi?bangumiId=xxx",
  "callback_url": "http://core-service:8000/raw-fetcher-results"
}
```

**Response (202 Accepted):**
```json
{
  "accepted": true,
  "message": "Fetch task accepted for subscription 123"
}
```

### GET /health
Health check endpoint.

**Response (200 OK):**
```json
{
  "status": "ok"
}
```

### POST /can-handle-subscription
Check if this fetcher can handle a given subscription URL.

**Request:**
```json
{
  "source_url": "https://mikanani.me/RSS/Bangumi?bangumiId=xxx",
  "source_type": "rss"
}
```

**Response (200 OK - can handle):**
```json
{
  "can_handle": true
}
```

**Response (204 No Content - cannot handle):**
```json
{
  "can_handle": false
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CORE_SERVICE_URL` | `http://core-service:8000` | Core service address for service registration |
| `SERVICE_HOST` | `fetcher-mikanani` | This service's hostname |
| `SERVICE_PORT` | `8001` | This service's port |
| `ENABLE_CORS` | `true` | Enable CORS middleware |
| `RUST_LOG` | `fetcher_mikanani=debug` | Log level configuration |

## Usage

### Docker

```bash
# Build the fetcher service
docker compose build fetcher-mikanani

# Start the fetcher service
docker compose up fetcher-mikanani

# Start with core service
docker compose up core-service fetcher-mikanani
```

### Local Development

```bash
# Set up environment
export CORE_SERVICE_URL=http://localhost:8000
export RUST_LOG=fetcher_mikanani=debug

# Run the service
cargo run --package fetcher-mikanani
```

### Testing

```bash
# Run all tests
cargo test --package fetcher-mikanani

# Run tests with output
cargo test --package fetcher-mikanani -- --nocapture

# Run specific test module
cargo test --package fetcher-mikanani handlers
```

## Architecture

The fetcher service consists of:

- **rss_parser.rs**: Core RSS parsing logic using feed-rs
  - Parses RSS feed items
  - Extracts raw anime item data

- **retry.rs**: Generic retry mechanism with exponential backoff
  - Configurable retry attempts
  - Exponential backoff

- **fetch_task.rs**: Background fetch task execution
  - Async fetch execution
  - Callback to core service

- **handlers.rs**: HTTP request/response handlers
  - POST /fetch endpoint
  - GET /health endpoint
  - POST /can-handle-subscription endpoint

- **http_client.rs**: HTTP client abstraction
  - Trait-based design for testability
  - Mock client for unit tests

- **config.rs**: Service configuration
  - Environment variable loading
  - URL construction helpers

- **cors.rs**: CORS middleware configuration
  - Configurable via environment variables

- **main.rs**: Service initialization and registration
  - Service startup
  - Service registration with core service
  - HTTP server setup

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| feed-rs | 2.3 | RSS feed parsing |
| reqwest | - | Async HTTP client |
| axum | - | Web framework |
| tokio | - | Async runtime |
| serde | - | Serialization |
| serde_json | - | JSON handling |
| tower-http | - | CORS middleware |

## Testing

Test coverage includes:

### Unit Tests (24 tests)

- **Config Tests** (2 tests)
  - URL construction (callback_url, register_url)

- **HTTP Client Tests** (3 tests)
  - Mock client request recording
  - Response configuration
  - Error simulation

- **Retry Tests** (5 tests)
  - First attempt success
  - Second attempt success
  - Retry exhaustion
  - Exponential backoff verification
  - Multiple failure recovery

- **Fetch Task Tests** (5 tests)
  - Payload serialization
  - Callback success/error handling
  - Parse error handling

- **Handler Tests** (5 tests)
  - Health check returns OK
  - Can handle mikanani RSS
  - Cannot handle other RSS
  - Cannot handle non-RSS types
  - Fetch returns 202 Accepted

- **CORS Tests** (2 tests)
  - CORS enabled/disabled

- **Main Tests** (2 tests)
  - Service registration
  - Registration error handling

Run tests with:
```bash
cargo test --package fetcher-mikanani
```

## Data Flow

1. **Core Service** schedules a fetch and calls `POST /fetch` on fetcher
2. **Fetcher** returns 202 Accepted immediately
3. **Fetcher** spawns background task to fetch RSS
4. **Fetcher** parses RSS and extracts raw items
5. **Fetcher** calls back to core service at `POST /raw-fetcher-results`
6. **Core Service** processes and stores the results

## Deployment

### Docker Compose Configuration

The service is configured in `docker compose.yml`:

```yaml
fetcher-mikanani:
  build:
    context: .
    dockerfile: Dockerfile.fetcher-mikanani
  container_name: bangumi-fetcher-mikanani
  ports:
    - "8001:8001"
  environment:
    - CORE_SERVICE_URL=http://core-service:8000
    - SERVICE_HOST=fetcher-mikanani
    - SERVICE_PORT=8001
    - RUST_LOG=fetcher_mikanani=debug
  depends_on:
    core-service:
      condition: service_healthy
  networks:
    - bangumi-network
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:8001/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 10s
```

## Troubleshooting

### Service fails to start

Check logs:
```bash
docker compose logs fetcher-mikanani
```

Verify core service is running:
```bash
curl http://localhost:8000/health
```

### Health check failing

Ensure curl is available in the container.

### Registration not working

Verify `CORE_SERVICE_URL` environment variable is set correctly and core service is healthy.

## Status

✅ **Production Ready**
- All 24 tests passing
- Comprehensive error handling
- Full API documentation
- Docker deployment verified
- Service registration working
- Health checks operational
- Retry logic tested

## Related Services

- **core-service**: Central service registry and coordination
- **downloader-qbittorrent**: Torrent download service
- **viewer-jellyfin**: Media library service

## Contributing

When making changes to the fetcher:

1. Add tests for new functionality
2. Run full test suite: `cargo test --package fetcher-mikanani`
3. Verify Docker build: `docker compose build fetcher-mikanani`
4. Test deployment: `docker compose up core-service fetcher-mikanani`

## License

Part of the rust-bangumi project.
