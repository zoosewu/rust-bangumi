# Jellyfin Viewer Service

A Rust-based service for organizing anime files into a Jellyfin-compatible media library structure. This service handles episode file organization, metadata management, and synchronization with the core Bangumi service.

## Features

- **Automatic Episode Organization**: Organizes downloaded anime episodes into proper directory structures
- **Flexible Filename Patterns**: Supports both `S##E##` and `[##]` episode numbering formats
- **Safe Filesystem Operations**: Implements hard linking with fallback to file copying
- **RESTful API**: Provides endpoints for syncing and health checks
- **Service Registration**: Automatically registers with the core service
- **Comprehensive Logging**: Full tracing support for debugging and monitoring

## Architecture

### Core Components

#### FileOrganizer
Responsible for file organization and episode information extraction.

- **`organize_episode()`**: Organizes a single episode file into the library structure
- **`extract_episode_info()`**: Parses filenames to extract season and episode numbers
- **`sanitize_filename()`**: Ensures filenames are safe for all filesystems

#### Handlers
HTTP request handlers for external API interactions.

- **`sync()`**: Syncs multiple episodes to the media library
- **`health_check()`**: Reports service health status

### Directory Structure

```
/media/jellyfin/
├── Anime Title/
│   ├── Season 01/
│   │   ├── Anime Title - S01E01.mkv
│   │   ├── Anime Title - S01E02.mkv
│   │   └── ...
│   └── Season 02/
│       └── Anime Title - S02E01.mkv
└── Another Anime/
    └── Season 01/
        └── ...
```

## API Documentation

### POST /sync

Synchronizes anime episodes from the downloads directory to the Jellyfin library.

**Request:**
```json
{
  "anime_id": 123,
  "anime_title": "Attack on Titan",
  "season": 1,
  "episodes": [
    {
      "episode_number": 1,
      "file_path": "/downloads/anime_s01e01.mkv"
    },
    {
      "episode_number": 2,
      "file_path": "/downloads/anime_s01e02.mkv"
    }
  ]
}
```

**Response (Success - 200 OK):**
```json
{
  "status": "success",
  "count": 2,
  "organized_files": [
    {
      "episode_number": 1,
      "source_path": "/downloads/anime_s01e01.mkv",
      "target_path": "/media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E01.mkv"
    },
    {
      "episode_number": 2,
      "source_path": "/downloads/anime_s01e02.mkv",
      "target_path": "/media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E02.mkv"
    }
  ],
  "error": null
}
```

**Response (Partial Failure - 500 Internal Server Error):**
```json
{
  "status": "partial_failure",
  "count": 1,
  "organized_files": [
    {
      "episode_number": 1,
      "source_path": "/downloads/anime_s01e01.mkv",
      "target_path": "/media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E01.mkv"
    }
  ],
  "error": "Source file does not exist: /downloads/anime_s01e02.mkv"
}
```

### GET /health

Health check endpoint for monitoring service status.

**Response (200 OK):**
```json
{
  "status": "healthy",
  "service": "jellyfin-viewer",
  "version": "0.1.0"
}
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DOWNLOADS_DIR` | `/downloads` | Source directory for downloaded episodes |
| `JELLYFIN_LIBRARY_DIR` | `/media/jellyfin` | Target Jellyfin library directory |
| `CORE_SERVICE_URL` | `http://core-service:8000` | URL of the core service for registration |
| `RUST_LOG` | `viewer_jellyfin=debug` | Logging level configuration |

### Example Configuration

```bash
export DOWNLOADS_DIR=/path/to/downloads
export JELLYFIN_LIBRARY_DIR=/path/to/jellyfin/library
export CORE_SERVICE_URL=http://localhost:8000
```

## Filename Formats Supported

### Format 1: Standard Episode Format (S##E##)

```
anime_s01e01.mkv
anime_S05E12.mkv
Show_S02E03.mp4
```

### Format 2: Bracket Format ([##])

```
anime_[01].mkv
show_[12].mkv
```

The service supports case-insensitive matching and extracts:
- Season and episode numbers from filenames
- File extensions (preserves original format)
- Anime title from the title parameter

## Filename Sanitization

Special characters that are unsafe for filesystems are replaced with underscores:

| Character | Replacement | Example |
|-----------|------------|---------|
| `/` `\` | `_` | `Demon/Slayer` → `Demon_Slayer` |
| `:` | `_` | `Title: Subtitle` → `Title_ Subtitle` |
| `*` `?` | `_` | `What*When?` → `What_When_` |
| `"` `<` `>` | `_` | `"Quote"` → `_Quote_` |
| `\|` | `_` | `A\|B` → `A_B` |

## File Operations

### Hard Linking Strategy

The service attempts to hard link files first for efficiency:

1. **Attempt Hard Link**: Creates a hard link to the source file
   - Zero-copy operation
   - Preserves original file
   - More efficient use of disk space

2. **Fallback to Copy**: If hard linking fails
   - Falls back to file copy
   - Ensures compatibility across different filesystems
   - Maintains data integrity

This strategy provides optimal performance while ensuring broad filesystem compatibility.

## Docker Configuration

### Docker Compose Example

```yaml
version: '3.8'

services:
  jellyfin-viewer:
    build: .
    container_name: viewer-jellyfin
    environment:
      - DOWNLOADS_DIR=/downloads
      - JELLYFIN_LIBRARY_DIR=/media/jellyfin
      - CORE_SERVICE_URL=http://core-service:8000
      - RUST_LOG=viewer_jellyfin=debug
    ports:
      - "8003:8003"
    volumes:
      - ./downloads:/downloads
      - ./media/jellyfin:/media/jellyfin
    depends_on:
      - core-service
    networks:
      - bangumi-network

networks:
  bangumi-network:
    driver: bridge
```

### Dockerfile

```dockerfile
FROM rust:1.75 AS builder

WORKDIR /build
COPY . .

RUN cargo build --release -p viewer-jellyfin

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/viewer-jellyfin /usr/local/bin/

EXPOSE 8003

CMD ["viewer-jellyfin"]
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test -p viewer-jellyfin

# Run with output
cargo test -p viewer-jellyfin -- --nocapture

# Run specific test
cargo test -p viewer-jellyfin test_sanitize_filename
```

### Test Coverage

The test suite includes:

1. **Unit Tests** (in module files):
   - Filename sanitization
   - Episode info extraction
   - File organizer initialization
   - Request/response serialization

2. **Integration Tests** (in `tests/viewer_tests.rs`):
   - Path construction and joining
   - Multiple character sanitization scenarios
   - Regex pattern matching for episode formats
   - JSON serialization/deserialization
   - Error handling and edge cases
   - Service registration structure validation

### Example Test Output

```
running 16 tests
test file_organizer::tests::test_sanitize_filename ... ok
test file_organizer::tests::test_extract_episode_info_s_e_format ... ok
test file_organizer::tests::test_extract_episode_info_bracket_format ... ok
test file_organizer::tests::test_extract_episode_info_no_match ... ok
test file_organizer::tests::test_file_organizer_creation ... ok
test handlers::tests::test_sync_request_deserialization ... ok
test handlers::tests::test_sync_response_serialization ... ok
test handlers::tests::test_health_response ... ok
test viewer_tests::test_path_construction ... ok
test viewer_tests::test_filename_sanitization_various_chars ... ok
test viewer_tests::test_episode_regex_s_e_format ... ok
test viewer_tests::test_sync_request_json_structure ... ok
test viewer_tests::test_sync_response_json_structure ... ok
test viewer_tests::test_health_check_response ... ok
test viewer_tests::test_episode_number_formatting ... ok
test viewer_tests::test_service_registration_structure ... ok

test result: ok. 16 passed
```

## Usage Examples

### Basic Synchronization

```rust
use viewer_jellyfin::file_organizer::FileOrganizer;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let organizer = FileOrganizer::new(
        PathBuf::from("/downloads"),
        PathBuf::from("/media/jellyfin"),
    );

    let result = organizer.organize_episode(
        "Attack on Titan",
        1,
        1,
        std::path::Path::new("/downloads/anime_s01e01.mkv"),
    ).await?;

    println!("Organized to: {}", result.display());
    Ok(())
}
```

### Extracting Episode Information

```rust
let organizer = FileOrganizer::new(
    PathBuf::from("/downloads"),
    PathBuf::from("/media/jellyfin"),
);

if let Some((season, episode)) = organizer.extract_episode_info("anime_s01e05.mkv") {
    println!("Season: {}, Episode: {}", season, episode);
}
```

## Error Handling

The service handles various error scenarios:

- **File Not Found**: Returns error if source file doesn't exist
- **Permission Denied**: Logs error and returns partial failure response
- **Disk Space**: Both hard link and copy operations handle disk full conditions
- **Invalid Paths**: Sanitizes and validates all path components
- **Network Issues**: Gracefully handles registration failures with core service

## Performance Characteristics

- **Hard Link**: O(1) time, zero additional disk space
- **File Copy**: O(n) time where n is file size, requires n additional disk space
- **Path Sanitization**: O(n) where n is title length
- **Regex Parsing**: O(n) where n is filename length, optimized with lazy_static

## Logging

The service uses the `tracing` crate for structured logging:

```
2024-01-22T10:30:45.123Z INFO viewer_jellyfin: Starting Jellyfin Viewer Service
2024-01-22T10:30:45.456Z INFO viewer_jellyfin: File organizer initialized
2024-01-22T10:30:45.789Z INFO viewer_jellyfin: Successfully registered with core service
2024-01-22T10:30:46.012Z INFO viewer_jellyfin: Jellyfin Viewer Service listening on 0.0.0.0:8003
2024-01-22T10:30:50.123Z INFO viewer_jellyfin: Organized: /downloads/anime_s01e01.mkv -> /media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E01.mkv
```

## Contributing

When contributing to this service:

1. Ensure all tests pass: `cargo test -p viewer-jellyfin`
2. Follow Rust conventions and run `cargo fmt`
3. Run clippy for code quality: `cargo clippy -p viewer-jellyfin`
4. Add tests for new functionality
5. Update this README with significant changes

## Dependencies

- **axum**: Web framework and routing
- **tokio**: Async runtime
- **serde**: Serialization framework
- **regex**: Episode format parsing
- **tracing**: Structured logging
- **anyhow**: Error handling

## License

MIT - See LICENSE file in repository root

## Version

0.1.0

## Author

Claude Code

## Support

For issues or questions, please refer to the main Bangumi project repository.
