//! Dashboard overview stats API

use axum::{extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use serde_json::json;

use crate::dto::{DashboardStats, ServiceInfo};
use crate::schema::{
    anime_links, anime_works, animes, downloads, raw_anime_items, service_modules,
    subscription_conflicts, subscriptions,
};
use crate::state::AppState;

/// Get dashboard statistics
#[utoipa::path(
    get,
    path = "/api/core/dashboard/stats",
    tag = "Dashboard",
    responses(
        (status = 200, description = "Success", body = DashboardStats),
        (status = 500, description = "Database error")
    )
)]
pub async fn get_dashboard_stats(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    // Only count anime_works that have at least one link (matches AnimePage has_links=true filter)
    let total_anime: i64 = anime_works::table
        .filter(diesel::dsl::exists(
            animes::table
                .inner_join(anime_links::table.on(anime_links::anime_id.eq(animes::anime_id)))
                .filter(animes::work_id.eq(anime_works::work_id))
                .select(anime_links::link_id),
        ))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    // Only count anime series that have at least one unfiltered link (matches AnimeSeriesPage excludeEmpty=true filter)
    let total_series: i64 = animes::table
        .filter(diesel::dsl::exists(
            anime_links::table
                .filter(anime_links::anime_id.eq(animes::anime_id))
                .filter(anime_links::filtered_flag.eq(false))
                .select(anime_links::link_id),
        ))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let active_subscriptions: i64 = subscriptions::table
        .filter(subscriptions::is_active.eq(true))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let total_downloads: i64 = downloads::table
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let downloading: i64 = downloads::table
        .filter(downloads::status.eq("downloading"))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let completed: i64 = downloads::table
        .filter(downloads::status.eq("completed"))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let failed: i64 = downloads::table
        .filter(downloads::status.eq("failed").or(downloads::status.eq("no_downloader")))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let pending_raw_items: i64 = raw_anime_items::table
        .filter(raw_anime_items::status.eq("pending"))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    let pending_conflicts: i64 = subscription_conflicts::table
        .filter(subscription_conflicts::resolution_status.eq("pending"))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    // Service health info from registry
    let services: Vec<ServiceInfo> = match service_modules::table
        .filter(service_modules::is_enabled.eq(true))
        .select((
            service_modules::name,
            service_modules::module_type,
        ))
        .load::<(String, crate::models::ModuleTypeEnum)>(&mut conn)
    {
        Ok(mods) => mods
            .into_iter()
            .map(|(name, module_type)| {
                let is_healthy = state
                    .registry
                    .get_services()
                    .unwrap_or_default()
                    .iter()
                    .any(|s| s.service_name == name && s.is_healthy);
                ServiceInfo {
                    name,
                    module_type: module_type.to_string(),
                    is_healthy,
                }
            })
            .collect(),
        Err(_) => vec![],
    };

    let stats = DashboardStats {
        total_anime,
        total_series,
        active_subscriptions,
        total_downloads,
        downloading,
        completed,
        failed,
        pending_raw_items,
        pending_conflicts,
        services,
    };

    (StatusCode::OK, Json(json!(stats)))
}
