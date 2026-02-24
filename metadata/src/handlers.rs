use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use crate::{
    bangumi_client::BangumiClient,
    models::{EnrichAnimeRequest, EnrichAnimeResponse, EnrichEpisodesRequest, EnrichEpisodesResponse},
};

#[derive(Clone)]
pub struct AppState {
    pub bangumi: Arc<BangumiClient>,
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}

pub async fn enrich_anime(
    State(state): State<AppState>,
    Json(req): Json<EnrichAnimeRequest>,
) -> impl IntoResponse {
    let bangumi_id = match state.bangumi.search_anime(&req.title).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(EnrichAnimeResponse {
                bangumi_id: None,
                cover_images: vec![],
                summary: None,
                air_date: None,
            })
            .into_response();
        }
        Err(e) => {
            tracing::warn!("Bangumi search failed for '{}': {}", req.title, e);
            return Json(EnrichAnimeResponse {
                bangumi_id: None,
                cover_images: vec![],
                summary: None,
                air_date: None,
            })
            .into_response();
        }
    };

    let cover_images = state
        .bangumi
        .get_cover_images(bangumi_id)
        .await
        .unwrap_or_default();

    let meta = state
        .bangumi
        .get_subject_meta(bangumi_id)
        .await
        .unwrap_or(crate::bangumi_client::SubjectMeta {
            summary: None,
            air_date: None,
        });

    Json(EnrichAnimeResponse {
        bangumi_id: Some(bangumi_id),
        cover_images,
        summary: meta.summary,
        air_date: meta.air_date,
    })
    .into_response()
}

pub async fn enrich_episodes(
    State(state): State<AppState>,
    Json(req): Json<EnrichEpisodesRequest>,
) -> impl IntoResponse {
    match state.bangumi.get_episode(req.bangumi_id, req.episode_no).await {
        Ok(Some(ep)) => Json(EnrichEpisodesResponse {
            episode_no: ep.episode_no,
            title: ep.title,
            title_cn: ep.title_cn,
            air_date: ep.air_date,
            summary: ep.summary,
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("get_episode failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
