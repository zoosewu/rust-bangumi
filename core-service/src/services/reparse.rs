//! Reparse 業務邏輯：重新解析 raw_anime_items 並 upsert anime_links

use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;

use crate::db::DbPool;
use crate::models::{AnimeLink, Download, NewAnimeLink, RawAnimeItem};
use crate::schema::{anime_links, raw_anime_items};
use crate::services::title_parser::{ParseStatus, ParsedResult, TitleParserService};
use crate::services::{
    ConflictDetectionService, DownloadCancelService, DownloadDispatchService, SyncService,
};

#[derive(Debug, Serialize, Default)]
pub struct ReparseStats {
    pub total: usize,
    pub parsed: usize,
    pub failed: usize,
    pub no_match: usize,
    pub resync_triggered: usize,
}

struct UpsertResult {
    link_id: i32,
    is_new: bool,
    metadata_changed: bool,
}

/// 重新解析所有 raw_anime_items（無論原始狀態）
pub async fn reparse_all_items(
    db: DbPool,
    dispatch_service: Arc<DownloadDispatchService>,
    sync_service: Arc<SyncService>,
    conflict_detection: Arc<ConflictDetectionService>,
    cancel_service: Arc<DownloadCancelService>,
) -> ReparseStats {
    let all_ids = {
        let mut conn = match db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("reparse_all_items: 無法取得 DB 連線: {}", e);
                return ReparseStats::default();
            }
        };
        match raw_anime_items::table
            .select(raw_anime_items::item_id)
            .load::<i32>(&mut conn)
        {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!("reparse_all_items: 查詢項目失敗: {}", e);
                return ReparseStats::default();
            }
        }
    };

    if all_ids.is_empty() {
        tracing::info!("reparse_all_items: 沒有任何項目");
        return ReparseStats::default();
    }

    tracing::info!("reparse_all_items: 開始重新解析全部 {} 筆項目", all_ids.len());
    reparse_affected_items(
        db,
        dispatch_service,
        sync_service,
        conflict_detection,
        cancel_service,
        &all_ids,
    )
    .await
}

/// 重新解析指定的 raw_anime_items
///
/// 使用 upsert 邏輯：更新既有的 anime_link 而非刪除重建，
/// 確保 downloads 記錄不會因 CASCADE 被刪除。
///
/// 1. 載入指定項目
/// 2. 對每筆項目重新解析
/// 3. 如果已有 anime_link → 更新欄位（保留 link_id 及關聯的 downloads）
/// 4. 如果沒有 anime_link → 新建
/// 5. 如果無匹配 → 刪除既有 anime_link（此項目本來就沒有成功的下載）並更新狀態
pub async fn reparse_affected_items(
    db: DbPool,
    dispatch_service: Arc<DownloadDispatchService>,
    sync_service: Arc<SyncService>,
    conflict_detection: Arc<ConflictDetectionService>,
    cancel_service: Arc<DownloadCancelService>,
    item_ids: &[i32],
) -> ReparseStats {
    if item_ids.is_empty() {
        return ReparseStats::default();
    }

    let mut conn = match db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("reparse_affected_items: 無法取得 DB 連線: {}", e);
            return ReparseStats::default();
        }
    };

    // 載入項目
    let items: Vec<RawAnimeItem> = match raw_anime_items::table
        .filter(raw_anime_items::item_id.eq_any(item_ids))
        .load::<RawAnimeItem>(&mut conn)
    {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("reparse_affected_items: 載入項目失敗: {}", e);
            return ReparseStats::default();
        }
    };

    tracing::info!(
        "reparse_affected_items: 開始重新解析 {} 筆項目",
        items.len()
    );

    let total = items.len();
    let mut parsed_count = 0;
    let mut failed_count = 0;
    let mut no_match_count = 0;
    let mut new_link_ids: Vec<i32> = Vec::new();
    let mut updated_link_ids: Vec<i32> = Vec::new();
    let mut unmatched_link_ids: Vec<i32> = Vec::new(); // links to cancel + delete
    let mut unmatched_series_ids: Vec<(i32, i32)> = Vec::new(); // (item_id, series_id) for cleanup
    let mut resync_link_ids: Vec<i32> = Vec::new();

    for item in &items {
        match TitleParserService::parse_title(&mut conn, &item.title) {
            Ok(Some(parsed)) => {
                match upsert_anime_link(&mut conn, item, &parsed) {
                    Ok(result) => {
                        if result.is_new {
                            new_link_ids.push(result.link_id);
                        } else {
                            updated_link_ids.push(result.link_id);
                        }
                        if result.metadata_changed {
                            resync_link_ids.push(result.link_id);
                        }
                        let is_new = result.is_new;
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            item.item_id,
                            ParseStatus::Parsed,
                            Some(parsed.parser_id),
                            None,
                        )
                        .ok();
                        parsed_count += 1;
                        tracing::info!(
                            "reparse: {} -> {} EP{} ({})",
                            item.title,
                            parsed.anime_title,
                            parsed.episode_no,
                            if is_new { "new" } else { "updated" }
                        );
                    }
                    Err(e) => {
                        TitleParserService::update_raw_item_status(
                            &mut conn,
                            item.item_id,
                            ParseStatus::Failed,
                            Some(parsed.parser_id),
                            Some(&e),
                        )
                        .ok();
                        failed_count += 1;
                        tracing::warn!("reparse: 建立/更新記錄失敗 {}: {}", item.title, e);
                    }
                }
            }
            Ok(None) => {
                // 無匹配：收集舊 link 資訊，稍後統一取消下載再刪除
                let old_link: Option<(i32, i32)> = anime_links::table
                    .filter(anime_links::raw_item_id.eq(item.item_id))
                    .select((anime_links::link_id, anime_links::anime_id))
                    .first::<(i32, i32)>(&mut conn)
                    .optional()
                    .ok()
                    .flatten();
                if let Some((lid, sid)) = old_link {
                    unmatched_link_ids.push(lid);
                    unmatched_series_ids.push((item.item_id, sid));
                }
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    ParseStatus::NoMatch,
                    None,
                    None,
                )
                .ok();
                no_match_count += 1;
            }
            Err(e) => {
                TitleParserService::update_raw_item_status(
                    &mut conn,
                    item.item_id,
                    ParseStatus::Failed,
                    None,
                    Some(&e),
                )
                .ok();
                failed_count += 1;
                tracing::warn!("reparse: 解析錯誤 {}: {}", item.title, e);
            }
        }
    }

    tracing::info!(
        "reparse_affected_items: 完成，共 {} 筆，成功 {}，失敗 {}，無匹配 {}",
        total,
        parsed_count,
        failed_count,
        no_match_count
    );

    // Cancel downloads for links that lost their match, BEFORE deleting the links
    // (downloads has ON DELETE CASCADE from anime_links, so we must cancel first)
    if !unmatched_link_ids.is_empty() {
        match cancel_service
            .cancel_downloads_for_links(&unmatched_link_ids)
            .await
        {
            Ok(n) => tracing::info!("reparse: cancelled {} downloads for unmatched links", n),
            Err(e) => tracing::warn!(
                "reparse: failed to cancel downloads for unmatched links: {}",
                e
            ),
        }

        // Now delete the unmatched links and clean up empty series
        if let Ok(mut del_conn) = db.get() {
            for &lid in &unmatched_link_ids {
                diesel::delete(anime_links::table.filter(anime_links::link_id.eq(lid)))
                    .execute(&mut del_conn)
                    .ok();
            }
            for &(_, sid) in &unmatched_series_ids {
                cleanup_empty_series(&mut del_conn, sid);
            }
        }
    }

    // 觸發衝突偵測
    let auto_dispatch_ids = match conflict_detection.detect_and_mark_conflicts().await {
        Ok(result) => result.auto_dispatch_link_ids,
        Err(e) => {
            tracing::warn!("reparse_affected_items: conflict detection 失敗: {}", e);
            vec![]
        }
    };

    // Dispatch: new links + updated links (may have become eligible) + auto-resolved conflict links
    // dispatch_new_links will automatically skip filtered/conflicted/already-downloading links
    let mut to_dispatch = new_link_ids;
    to_dispatch.extend(updated_link_ids);
    to_dispatch.extend(auto_dispatch_ids);
    to_dispatch.sort_unstable();
    to_dispatch.dedup();
    if !to_dispatch.is_empty() {
        match dispatch_service.dispatch_new_links(to_dispatch).await {
            Ok(r) => tracing::info!(
                "reparse: dispatched {} links, {} no_downloader, {} failed",
                r.dispatched,
                r.no_downloader,
                r.failed
            ),
            Err(e) => tracing::warn!("reparse_affected_items: dispatch 失敗: {}", e),
        }
    }

    // 觸發 resync（metadata 變更的已 synced downloads）
    let mut resync_triggered = 0;
    if !resync_link_ids.is_empty() {
        let mut conn_for_resync = match db.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("reparse: 無法取得 DB 連線用於 resync: {}", e);
                return ReparseStats {
                    total,
                    parsed: parsed_count,
                    failed: failed_count,
                    no_match: no_match_count,
                    resync_triggered,
                };
            }
        };

        // Find synced downloads for these links
        let synced_downloads: Vec<Download> = crate::schema::downloads::table
            .filter(crate::schema::downloads::link_id.eq_any(&resync_link_ids))
            .filter(crate::schema::downloads::status.eq("synced"))
            .filter(crate::schema::downloads::file_path.is_not_null())
            .load::<Download>(&mut conn_for_resync)
            .unwrap_or_default();

        drop(conn_for_resync);

        if !synced_downloads.is_empty() {
            tracing::info!(
                "reparse: 偵測到 {} 筆已 synced 的 downloads 需要 resync",
                synced_downloads.len()
            );
            for download in &synced_downloads {
                match sync_service.notify_viewer_resync(download).await {
                    Ok(true) => {
                        resync_triggered += 1;
                        tracing::info!(
                            "reparse: resync 通知已發送 download_id={}",
                            download.download_id
                        );
                    }
                    Ok(false) => {
                        tracing::warn!(
                            "reparse: 無 viewer 可用於 resync download_id={}",
                            download.download_id
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "reparse: resync 失敗 download_id={}: {}",
                            download.download_id,
                            e
                        );
                    }
                }
            }
        }
    }

    ReparseStats {
        total,
        parsed: parsed_count,
        failed: failed_count,
        no_match: no_match_count,
        resync_triggered,
    }
}

/// 建立或更新 anime_link。
/// 如果此 raw_item 已有 anime_link → 更新欄位（保留 link_id 及 downloads）。
/// 如果沒有 → 新建。
/// 回傳 (link_id, is_new)。
fn upsert_anime_link(
    conn: &mut diesel::PgConnection,
    raw_item: &RawAnimeItem,
    parsed: &ParsedResult,
) -> Result<UpsertResult, String> {
    use sha2::{Digest, Sha256};

    // 1. 建立或取得 anime / season / series / group
    let anime =
        crate::handlers::fetcher_results::create_or_get_anime(conn, &parsed.anime_title)?;
    let year = parsed
        .year
        .as_ref()
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(2025);
    let season_name = parsed.season.as_deref().unwrap_or("unknown");
    let season =
        crate::handlers::fetcher_results::create_or_get_season(conn, year, season_name)?;
    let series = crate::handlers::fetcher_results::create_or_get_series(
        conn,
        anime.work_id,
        parsed.series_no,
        season.season_id,
        "",
    )?;
    let group_name = parsed.subtitle_group.as_deref().unwrap_or("未知字幕組");
    let group =
        crate::handlers::fetcher_results::create_or_get_subtitle_group(conn, group_name)?;

    // 2. 查找既有的 anime_link
    let existing_link: Option<AnimeLink> = anime_links::table
        .filter(anime_links::raw_item_id.eq(raw_item.item_id))
        .first(conn)
        .optional()
        .map_err(|e| format!("Failed to query existing link: {}", e))?;

    if let Some(link) = existing_link {
        let old_series_id = link.anime_id;
        let old_group_id = link.group_id;
        let old_episode_no = link.episode_no;

        // 3a. 更新既有 link（保留 link_id → downloads 不受影響）
        diesel::update(anime_links::table.filter(anime_links::link_id.eq(link.link_id)))
            .set((
                anime_links::anime_id.eq(series.anime_id),
                anime_links::group_id.eq(group.group_id),
                anime_links::episode_no.eq(parsed.episode_no),
                anime_links::title.eq(Some(&raw_item.title)),
            ))
            .execute(conn)
            .map_err(|e| format!("Failed to update anime link: {}", e))?;

        // anime 變更時，清理已無 link 的舊 anime
        if old_series_id != series.anime_id {
            cleanup_empty_series(conn, old_series_id);
        }

        // 重算 filtered_flag
        let updated_link: AnimeLink = anime_links::table
            .filter(anime_links::link_id.eq(link.link_id))
            .first(conn)
            .map_err(|e| format!("Failed to reload link: {}", e))?;
        if let Ok(flag) =
            crate::services::filter_recalc::compute_filtered_flag_for_link(conn, &updated_link)
        {
            if flag != updated_link.filtered_flag {
                diesel::update(
                    anime_links::table.filter(anime_links::link_id.eq(link.link_id)),
                )
                .set(anime_links::filtered_flag.eq(flag))
                .execute(conn)
                .ok();
            }
        }

        let metadata_changed = old_series_id != series.anime_id
            || old_group_id != group.group_id
            || old_episode_no != parsed.episode_no;

        Ok(UpsertResult {
            link_id: link.link_id,
            is_new: false,
            metadata_changed,
        })
    } else {
        // 3b. 新建 link
        let mut hasher = Sha256::new();
        hasher.update(raw_item.download_url.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        let now = Utc::now().naive_utc();
        let detected_type =
            crate::services::download_type_detector::detect_download_type(&raw_item.download_url);

        let new_link = NewAnimeLink {
            anime_id: series.anime_id,
            group_id: group.group_id,
            episode_no: parsed.episode_no,
            title: Some(raw_item.title.clone()),
            url: raw_item.download_url.clone(),
            source_hash,
            filtered_flag: false,
            created_at: now,
            raw_item_id: Some(raw_item.item_id),
            download_type: detected_type.map(|dt| dt.to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };

        let created_link: AnimeLink = diesel::insert_into(anime_links::table)
            .values(&new_link)
            .get_result(conn)
            .map_err(|e| format!("Failed to create anime link: {}", e))?;

        // 計算 filtered_flag
        if let Ok(flag) =
            crate::services::filter_recalc::compute_filtered_flag_for_link(conn, &created_link)
        {
            if flag != created_link.filtered_flag {
                diesel::update(
                    anime_links::table.filter(anime_links::link_id.eq(created_link.link_id)),
                )
                .set(anime_links::filtered_flag.eq(flag))
                .execute(conn)
                .ok();
            }
        }

        Ok(UpsertResult {
            link_id: created_link.link_id,
            is_new: true,
            metadata_changed: false,
        })
    }
}

/// 如果指定的 anime 底下已經沒有任何 anime_link，就刪除該 anime。
pub(crate) fn cleanup_empty_series(conn: &mut diesel::PgConnection, anime_id: i32) {
    use crate::schema::animes;

    let link_count: i64 = anime_links::table
        .filter(anime_links::anime_id.eq(anime_id))
        .count()
        .get_result(conn)
        .unwrap_or(1); // 查詢失敗時保守不刪

    if link_count == 0 {
        if let Err(e) = diesel::delete(animes::table.filter(animes::anime_id.eq(anime_id)))
            .execute(conn)
        {
            tracing::warn!("cleanup_empty_series: 刪除 anime {} 失敗: {}", anime_id, e);
        } else {
            tracing::info!("cleanup_empty_series: 已移除空的 anime {}", anime_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reparse_stats_default() {
        let stats = ReparseStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.parsed, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.no_match, 0);
        assert_eq!(stats.resync_triggered, 0);
    }

    #[test]
    fn test_reparse_stats_serializable() {
        let stats = ReparseStats {
            total: 10,
            parsed: 7,
            failed: 1,
            no_match: 2,
            resync_triggered: 0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"total\":10"));
        assert!(json.contains("\"parsed\":7"));
    }
}
