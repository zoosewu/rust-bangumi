use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use diesel::prelude::*;

use crate::state::AppState;
use crate::dto::{AnimeLinkRequest, AnimeLinkResponse};
use crate::models::{NewAnimeLink, AnimeLink};
use crate::schema::anime_links;

/// Create a new anime link
pub async fn create_anime_link(
    State(state): State<AppState>,
    Json(payload): Json<AnimeLinkRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let now = Utc::now().naive_utc();
    let new_link = NewAnimeLink {
        series_id: payload.series_id,
        group_id: payload.group_id,
        episode_no: payload.episode_no,
        title: payload.title,
        url: payload.url,
        source_hash: payload.source_hash,
        filtered_flag: false, // Default to false
        created_at: now,
        updated_at: now,
    };

    match state.db.get() {
        Ok(mut conn) => {
            match diesel::insert_into(anime_links::table)
                .values(&new_link)
                .get_result::<AnimeLink>(&mut conn)
            {
                Ok(link) => {
                    tracing::info!("Created anime link: {}", link.link_id);
                    let response = AnimeLinkResponse {
                        link_id: link.link_id,
                        series_id: link.series_id,
                        group_id: link.group_id,
                        episode_no: link.episode_no,
                        title: link.title,
                        url: link.url,
                        source_hash: link.source_hash,
                        created_at: link.created_at,
                        updated_at: link.updated_at,
                    };
                    (StatusCode::CREATED, Json(json!(response)))
                }
                Err(e) => {
                    tracing::error!("Failed to create anime link: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to create anime link: {}", e)
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e)
                })),
            )
        }
    }
}

/// Get anime links by series_id, only returning unfiltered links
pub async fn get_anime_links(
    State(state): State<AppState>,
    Path(series_id): Path<i32>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.db.get() {
        Ok(mut conn) => {
            match anime_links::table
                .filter(anime_links::series_id.eq(series_id))
                .filter(anime_links::filtered_flag.eq(false))
                .load::<AnimeLink>(&mut conn)
            {
                Ok(links) => {
                    let responses: Vec<AnimeLinkResponse> = links
                        .into_iter()
                        .map(|l| AnimeLinkResponse {
                            link_id: l.link_id,
                            series_id: l.series_id,
                            group_id: l.group_id,
                            episode_no: l.episode_no,
                            title: l.title,
                            url: l.url,
                            source_hash: l.source_hash,
                            created_at: l.created_at,
                            updated_at: l.updated_at,
                        })
                        .collect();
                    tracing::info!(
                        "Retrieved {} anime links for series_id={}",
                        responses.len(),
                        series_id
                    );
                    (StatusCode::OK, Json(json!({ "links": responses })))
                }
                Err(e) => {
                    tracing::error!("Failed to list anime links: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "database_error",
                            "message": format!("Failed to list anime links: {}", e),
                            "links": []
                        })),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "connection_error",
                    "message": format!("Failed to get database connection: {}", e),
                    "links": []
                })),
            )
        }
    }
}
