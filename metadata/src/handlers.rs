use crate::{
    bangumi_client::BangumiClient,
    models::{
        EnrichAnimeRequest, EnrichAnimeResponse, EnrichEpisodesRequest, EnrichEpisodesResponse,
        SeriesInfo,
    },
    series::series_title,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;

/// 這個服務能解析的 external id namespace
pub const NAMESPACE: &str = "bgm";

#[derive(Clone)]
pub struct AppState {
    pub bangumi: Arc<BangumiClient>,
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}

fn upstream_error(context: &str, e: anyhow::Error) -> axum::response::Response {
    tracing::error!("{}: {}", context, e);
    StatusCode::BAD_GATEWAY.into_response()
}

/// POST /enrich/anime — 一季的權威內容，以及它所屬系列（第一季）的名稱與內容
pub async fn enrich_anime(
    State(state): State<AppState>,
    Json(req): Json<EnrichAnimeRequest>,
) -> axum::response::Response {
    if req.namespace != NAMESPACE {
        return StatusCode::NOT_FOUND.into_response();
    }
    let season = match state.bangumi.get_subject(&req.external_id).await {
        Ok(Some(subject)) => subject,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return upstream_error("bgm subject lookup failed", e),
    };
    let chain = match state.bangumi.series_chain(season.clone()).await {
        Ok(chain) => chain,
        Err(e) => return upstream_error("bgm prequel lookup failed", e),
    };
    let root = chain.first().cloned().unwrap_or_else(|| season.clone());
    let series = SeriesInfo {
        namespace: NAMESPACE.to_string(),
        external_id: root.id.clone(),
        title: series_title(&chain),
        original_title: root.name.clone(),
        summary: root.summary.clone(),
        air_date: root.air_date.clone(),
        cover_images: root.cover_images.clone(),
    };
    Json(EnrichAnimeResponse {
        namespace: NAMESPACE.to_string(),
        external_id: season.id,
        title: season.name,
        title_cn: season.name_cn,
        summary: season.summary,
        air_date: season.air_date,
        end_date: season.end_date,
        total_episodes: season.total_episodes,
        platform: season.platform,
        cover_images: season.cover_images,
        series,
    })
    .into_response()
}

/// POST /enrich/episodes — 指定集數的內容
pub async fn enrich_episodes(
    State(state): State<AppState>,
    Json(req): Json<EnrichEpisodesRequest>,
) -> impl IntoResponse {
    if req.namespace != NAMESPACE {
        return StatusCode::NOT_FOUND.into_response();
    }
    match state
        .bangumi
        .get_episode(&req.external_id, req.episode_no)
        .await
    {
        Ok(Some(ep)) => Json(EnrichEpisodesResponse {
            episode_no: ep.episode_no,
            title: ep.title,
            title_cn: ep.title_cn,
            air_date: ep.air_date,
            summary: ep.summary,
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => upstream_error("get_episode failed", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::Path, routing::get, Router};
    use serde_json::{json, Value};

    /// 以實測資料為形狀的 bgm API：金牌得主兩季、魔法科劇場版（前傳是 TV）
    async fn mock_bgm() -> String {
        fn subject(id: &str) -> Option<Value> {
            match id {
                "430699" => Some(
                    json!({"id": 430699, "name": "メダリスト", "name_cn": "金牌得主",
                    "summary": "S1 簡介", "date": "2025-01-05", "platform": "TV", "eps": 13,
                    "images": {"large": "https://lain.bgm.tv/pic/cover/l/s1.jpg"}, "infobox": []}),
                ),
                "548818" => Some(
                    json!({"id": 548818, "name": "メダリスト 第2期", "name_cn": "金牌得主 第二季",
                    "summary": "S2 簡介", "date": "2026-01-24", "platform": "TV", "total_episodes": 9,
                    "images": {"large": "https://lain.bgm.tv/pic/cover/l/s2.jpg"},
                    "infobox": [{"key": "播放结束", "value": "2026年3月21日"}]}),
                ),
                "172494" => Some(
                    json!({"id": 172494, "name": "劇場版 魔法科高校の劣等生", "name_cn": "剧场版 魔法科高校的劣等生 呼唤星辰的少女",
                    "summary": "movie", "date": "2017-06-17", "platform": "剧场版", "images": {}, "infobox": []}),
                ),
                _ => None,
            }
        }
        fn relations(id: &str) -> Value {
            match id {
                "548818" => {
                    json!([{"id": 430699, "type": 2, "relation": "前传", "name_cn": "金牌得主"},
                                   {"id": 621939, "type": 2, "relation": "续集"}])
                }
                "172494" => json!([{"id": 84872, "type": 2, "relation": "前传"}]),
                _ => json!([]),
            }
        }
        let app = Router::new()
            .route(
                "/v0/subjects/:id",
                get(|Path(id): Path<String>| async move {
                    subject(&id)
                        .map(|s| Json(s).into_response())
                        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
                }),
            )
            .route(
                "/v0/subjects/:id/subjects",
                get(|Path(id): Path<String>| async move { Json(relations(&id)) }),
            )
            .route(
                "/v0/episodes",
                get(|| async {
                    Json(json!({"data": [
                        {"ep": 1, "sort": 14, "name": "選手宣誓", "name_cn": "选手宣誓", "airdate": "2026-01-24", "desc": "第一集"},
                        {"ep": 2, "sort": 15, "name": "", "name_cn": "我的手牌", "airdate": "2026-01-31", "desc": ""}
                    ]}))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{}", addr)
    }

    async fn state() -> AppState {
        AppState {
            bangumi: Arc::new(BangumiClient::with_base_url(&mock_bgm().await)),
        }
    }

    async fn enrich(state: AppState, namespace: &str, id: &str) -> axum::response::Response {
        enrich_anime(
            State(state),
            Json(EnrichAnimeRequest {
                namespace: namespace.to_string(),
                external_id: id.to_string(),
            }),
        )
        .await
    }

    async fn body(resp: axum::response::Response) -> EnrichAnimeResponse {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn a_later_season_reports_its_first_season_as_the_series() {
        let resp = body(enrich(state().await, "bgm", "548818").await).await;
        assert_eq!(resp.title_cn, "金牌得主 第二季");
        assert_eq!(resp.air_date.as_deref(), Some("2026-01-24"));
        assert_eq!(resp.end_date.as_deref(), Some("2026-03-21"));
        assert_eq!(resp.total_episodes, Some(9));
        assert_eq!(resp.series.external_id, "430699");
        assert_eq!(resp.series.title, "金牌得主");
        assert_eq!(resp.series.summary.as_deref(), Some("S1 簡介"));
        assert_eq!(resp.series.cover_images.len(), 1);
    }

    #[tokio::test]
    async fn a_first_season_is_its_own_series() {
        let resp = body(enrich(state().await, "bgm", "430699").await).await;
        assert_eq!(resp.series.external_id, "430699");
        assert_eq!(resp.series.title, "金牌得主");
    }

    #[tokio::test]
    async fn a_movie_stays_its_own_series() {
        let resp = body(enrich(state().await, "bgm", "172494").await).await;
        assert_eq!(resp.series.external_id, "172494");
        assert_eq!(
            resp.series.title,
            "剧场版 魔法科高校的劣等生 呼唤星辰的少女"
        );
    }

    #[tokio::test]
    async fn other_namespaces_and_unknown_subjects_are_not_found() {
        assert_eq!(
            enrich(state().await, "tmdb", "1").await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            enrich(state().await, "bgm", "999").await.status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn episodes_match_season_numbering_then_absolute_numbering() {
        let state = state().await;
        for (asked, expected) in [(1, "选手宣誓"), (15, "我的手牌")] {
            let ep = state
                .bangumi
                .get_episode("548818", asked)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(ep.title_cn.as_deref(), Some(expected));
        }
        assert!(state
            .bangumi
            .get_episode("548818", 3)
            .await
            .unwrap()
            .is_none());
    }
}
