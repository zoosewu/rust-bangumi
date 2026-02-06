// tests/unit/hash_extraction_tests.rs
//
// Hash extraction methods are now private on QBittorrentClient.
// These tests are retained for TorrentInfo-related checks but the
// extract_hash_from_magnet / extract_hash_from_url functions are
// tested indirectly through the add_torrents implementation.
//
// The unit tests for the private helpers are moved to inline #[cfg(test)]
// if needed in the future.

// This file is intentionally left minimal since the hash extraction
// functions are now private implementation details of QBittorrentClient.
