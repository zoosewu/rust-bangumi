use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::file_organizer::FileOrganizer;

#[derive(Debug, Deserialize)]
pub struct SyncRequest {
    #[allow(dead_code)]
    pub anime_id: i32,
    pub anime_title: String,
    pub season: u32,
    pub episodes: Vec<EpisodeInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EpisodeInfo {
    pub episode_number: u32,
    pub file_path: String,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub status: String,
    pub count: usize,
    pub organized_files: Vec<OrganizedFile>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrganizedFile {
    pub episode_number: u32,
    pub source_path: String,
    pub target_path: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

pub async fn sync(
    State(organizer): State<Arc<FileOrganizer>>,
    Json(req): Json<SyncRequest>,
) -> (StatusCode, Json<SyncResponse>) {
    let mut organized_files = Vec::new();
    let mut error = None;

    for episode in req.episodes {
        match organizer
            .organize_episode(
                &req.anime_title,
                req.season,
                episode.episode_number,
                std::path::Path::new(&episode.file_path),
            )
            .await
        {
            Ok(target_path) => {
                organized_files.push(OrganizedFile {
                    episode_number: episode.episode_number,
                    source_path: episode.file_path.clone(),
                    target_path: target_path.display().to_string(),
                });
                tracing::info!(
                    "Synced episode {} for anime: {}",
                    episode.episode_number,
                    req.anime_title
                );
            }
            Err(e) => {
                error = Some(e.to_string());
                tracing::error!(
                    "Failed to organize episode {} for anime {}: {}",
                    episode.episode_number,
                    req.anime_title,
                    e
                );
                break;
            }
        }
    }

    let response = SyncResponse {
        status: if error.is_none() {
            "success".to_string()
        } else {
            "partial_failure".to_string()
        },
        count: organized_files.len(),
        organized_files,
        error,
    };

    let status_code = if response.error.is_none() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };

    (status_code, Json(response))
}

pub async fn health_check() -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "healthy".to_string(),
            service: "jellyfin-viewer".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_request_deserialization() {
        let json = r#"{
            "anime_id": 123,
            "anime_title": "Attack on Titan",
            "season": 1,
            "episodes": [
                {"episode_number": 1, "file_path": "/path/to/episode1.mkv"}
            ]
        }"#;

        let req: SyncRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.anime_id, 123);
        assert_eq!(req.anime_title, "Attack on Titan");
        assert_eq!(req.season, 1);
        assert_eq!(req.episodes.len(), 1);
        assert_eq!(req.episodes[0].episode_number, 1);
    }

    #[test]
    fn test_sync_response_serialization() {
        let response = SyncResponse {
            status: "success".to_string(),
            count: 1,
            organized_files: vec![OrganizedFile {
                episode_number: 1,
                source_path: "/path/to/episode1.mkv".to_string(),
                target_path: "/media/jellyfin/Attack on Titan/Season 01/Attack on Titan - S01E01.mkv".to_string(),
            }],
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("Attack on Titan"));
    }

    #[test]
    fn test_health_response() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            service: "jellyfin-viewer".to_string(),
            version: "0.1.0".to_string(),
        };

        assert_eq!(response.status, "healthy");
        assert_eq!(response.service, "jellyfin-viewer");
    }
}
