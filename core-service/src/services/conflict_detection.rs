use std::sync::Arc;

use diesel::prelude::*;

use crate::db::repository::{AnimeLinkConflictRepository, AnimeLinkRepository};
use crate::db::DbPool;

/// Result of conflict detection, including links that need dispatch after auto-resolution.
pub struct ConflictDetectionResult {
    pub conflicts_found: usize,
    /// Link IDs that were restored to 'active' after conflict auto-resolution and need dispatch.
    pub auto_dispatch_link_ids: Vec<i32>,
}

/// 合集 torrent 涵蓋的每一集，在同一季都已有其他 link 的有效下載（不是 failed / cancelled），
/// 而合集本身還沒有在下載 → 把合集的 link 標為 resolved（不派送、不列為衝突）。
/// 只覆蓋部分集數的合集照常走衝突流程。回傳被略過的 link id。
pub fn skip_redundant_batches(conn: &mut diesel::PgConnection) -> QueryResult<Vec<i32>> {
    #[derive(QueryableByName)]
    struct Skipped {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        link_id: i32,
    }
    let mut skipped: Vec<i32> = diesel::sql_query(
        "WITH batch_links AS ( \
             SELECT l.link_id, l.raw_item_id, l.anime_id, l.episode_no \
             FROM anime_links l \
             WHERE l.link_status = 'active' AND l.filtered_flag = false \
               AND l.raw_item_id IS NOT NULL AND l.source_hash LIKE '%#ep%' \
         ), redundant AS ( \
             SELECT b.raw_item_id \
             FROM batch_links b \
             GROUP BY b.raw_item_id \
             HAVING bool_and( \
                 EXISTS (SELECT 1 FROM anime_links o JOIN downloads d ON d.link_id = o.link_id \
                         WHERE o.anime_id = b.anime_id AND o.episode_no = b.episode_no \
                           AND o.raw_item_id IS DISTINCT FROM b.raw_item_id \
                           AND d.status NOT IN ('failed', 'cancelled')) \
                 AND NOT EXISTS (SELECT 1 FROM downloads d \
                                 WHERE d.link_id = b.link_id \
                                   AND d.status NOT IN ('failed', 'cancelled'))) \
         ) \
         UPDATE anime_links SET link_status = 'resolved' \
         WHERE link_id IN (SELECT b.link_id FROM batch_links b JOIN redundant r USING (raw_item_id)) \
         RETURNING link_id",
    )
    .load::<Skipped>(conn)?
    .into_iter()
    .map(|s| s.link_id)
    .collect();
    skipped.sort_unstable();
    Ok(skipped)
}

pub struct ConflictDetectionService {
    link_repo: Arc<dyn AnimeLinkRepository>,
    conflict_repo: Arc<dyn AnimeLinkConflictRepository>,
    pool: Arc<DbPool>,
}

impl ConflictDetectionService {
    pub fn new(
        link_repo: Arc<dyn AnimeLinkRepository>,
        conflict_repo: Arc<dyn AnimeLinkConflictRepository>,
        pool: Arc<DbPool>,
    ) -> Self {
        Self {
            link_repo,
            conflict_repo,
            pool,
        }
    }

    /// Scan all active unfiltered anime_links, detect conflicts (same series_id, group_id,
    /// episode_no with COUNT > 1), set conflict_flag, and upsert anime_link_conflicts records.
    /// Also clears conflict_flag for groups that are no longer conflicting.
    /// When a conflict is auto-resolved, restores remaining links to 'active' status.
    pub async fn detect_and_mark_conflicts(&self) -> Result<ConflictDetectionResult, String> {
        let pool = self.pool.clone();
        let skipped = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            skip_redundant_batches(&mut conn).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("Batch skip task failed: {}", e))?
        .map_err(|e| format!("Failed to skip redundant batches: {}", e))?;
        if !skipped.is_empty() {
            tracing::info!(
                "Skipped {} links of batches whose episodes are all downloaded already",
                skipped.len()
            );
        }

        // First, clear all existing conflict_flags so stale conflicts are removed
        let cleared = self
            .link_repo
            .clear_all_conflict_flags()
            .await
            .map_err(|e| format!("Failed to clear conflict flags: {}", e))?;

        if cleared > 0 {
            tracing::debug!("Cleared {} stale conflict flags", cleared);
        }

        let conflict_groups = self
            .link_repo
            .detect_all_conflicts()
            .await
            .map_err(|e| format!("Failed to detect conflicts: {}", e))?;

        let mut conflicts_found = 0;

        for (anime_id, group_id, episode_no) in &conflict_groups {
            let links = self
                .link_repo
                .find_active_links_for_episode(*anime_id, *group_id, *episode_no)
                .await
                .map_err(|e| format!("Failed to find links for episode: {}", e))?;

            if links.len() > 1 {
                let link_ids: Vec<i32> = links.iter().map(|l| l.link_id).collect();

                // Set conflict_flag = true for all links in this group
                self.link_repo
                    .set_conflict_flags(&link_ids, true)
                    .await
                    .map_err(|e| format!("Failed to set conflict flags: {}", e))?;

                // Upsert conflict record
                self.conflict_repo
                    .upsert(*anime_id, *group_id, *episode_no)
                    .await
                    .map_err(|e| format!("Failed to upsert conflict: {}", e))?;

                conflicts_found += 1;

                tracing::info!(
                    "Conflict detected: anime_id={}, group_id={}, episode_no={}, {} links",
                    anime_id,
                    group_id,
                    episode_no,
                    links.len()
                );
            }
        }

        // Auto-resolve conflict records that no longer have actual conflicts
        let unresolved = self
            .conflict_repo
            .find_unresolved()
            .await
            .map_err(|e| format!("Failed to find unresolved conflicts: {}", e))?;

        let mut auto_resolved = 0;
        let mut auto_dispatch_link_ids: Vec<i32> = Vec::new();

        for conflict in &unresolved {
            let active_links = self
                .link_repo
                .find_active_links_for_episode(
                    conflict.anime_id,
                    conflict.group_id,
                    conflict.episode_no,
                )
                .await
                .map_err(|e| format!("Failed to check conflict links: {}", e))?;

            if active_links.len() <= 1 {
                // No longer a conflict — delete the conflict record
                let _ = self
                    .conflict_repo
                    .delete_by_episode(conflict.anime_id, conflict.group_id, conflict.episode_no)
                    .await;

                // Restore resolved links for this episode back to 'active'
                let resolved_links = self
                    .link_repo
                    .find_resolved_links_for_episode(
                        conflict.anime_id,
                        conflict.group_id,
                        conflict.episode_no,
                    )
                    .await
                    .map_err(|e| format!("Failed to find resolved links: {}", e))?;

                // Total links after restoration = active + resolved
                let total_after_restore = active_links.len() + resolved_links.len();

                if !resolved_links.is_empty() {
                    let resolved_ids: Vec<i32> = resolved_links.iter().map(|l| l.link_id).collect();
                    self.link_repo
                        .set_link_status(&resolved_ids, "active")
                        .await
                        .map_err(|e| format!("Failed to restore link status: {}", e))?;
                    tracing::info!(
                        "Restored {} resolved links to active for episode ({}, {}, {})",
                        resolved_ids.len(),
                        conflict.anime_id,
                        conflict.group_id,
                        conflict.episode_no
                    );
                }

                if total_after_restore > 1 {
                    // Restored links form a new conflict — re-flag them and create a new conflict record.
                    // Do NOT dispatch; user must resolve this new conflict manually.
                    let mut all_ids: Vec<i32> = active_links.iter().map(|l| l.link_id).collect();
                    all_ids.extend(resolved_links.iter().map(|l| l.link_id));
                    self.link_repo
                        .set_conflict_flags(&all_ids, true)
                        .await
                        .map_err(|e| format!("Failed to set conflict flags: {}", e))?;
                    self.conflict_repo
                        .upsert(conflict.anime_id, conflict.group_id, conflict.episode_no)
                        .await
                        .map_err(|e| format!("Failed to upsert conflict: {}", e))?;
                    tracing::info!(
                        "Restored links form new conflict for episode ({}, {}, {}), {} links",
                        conflict.anime_id,
                        conflict.group_id,
                        conflict.episode_no,
                        total_after_restore
                    );
                    conflicts_found += 1;
                } else {
                    // Only 0 or 1 link total — safe to dispatch
                    for link in &active_links {
                        auto_dispatch_link_ids.push(link.link_id);
                    }
                    for link in &resolved_links {
                        auto_dispatch_link_ids.push(link.link_id);
                    }
                }

                auto_resolved += 1;
            }
        }

        if auto_resolved > 0 {
            tracing::info!(
                "Auto-resolved {} conflict records (no longer conflicting after filter change)",
                auto_resolved,
            );
        }

        if conflicts_found > 0 {
            tracing::info!(
                "Conflict detection complete: {} conflict groups found",
                conflicts_found
            );
        }

        Ok(ConflictDetectionResult {
            conflicts_found,
            auto_dispatch_link_ids,
        })
    }

    /// Bulk-resolve all unresolved conflicts under a given (anime_id, group_id) by
    /// preferring links from the chosen `raw_item_id`. For each conflict that has a
    /// candidate link with the matching raw_item_id, that link is chosen; others
    /// in the same conflict become `link_status='resolved'`. Conflicts with no
    /// candidate from `chosen_raw_item_id` are skipped.
    ///
    /// Returns: (resolved_conflict_ids, skipped_conflict_ids, chosen_link_ids,
    /// unchosen_link_ids).
    pub async fn resolve_conflicts_by_raw_item(
        &self,
        anime_id: i32,
        group_id: i32,
        chosen_raw_item_id: i32,
    ) -> Result<(Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>), String> {
        let unresolved = self
            .conflict_repo
            .find_unresolved()
            .await
            .map_err(|e| format!("Failed to find unresolved conflicts: {}", e))?;

        let mut resolved_ids: Vec<i32> = Vec::new();
        let mut skipped_ids: Vec<i32> = Vec::new();
        let mut chosen_links: Vec<i32> = Vec::new();
        let mut unchosen_links: Vec<i32> = Vec::new();

        for conflict in unresolved
            .into_iter()
            .filter(|c| c.anime_id == anime_id && c.group_id == group_id)
        {
            let links = self
                .link_repo
                .find_active_links_for_episode(
                    conflict.anime_id,
                    conflict.group_id,
                    conflict.episode_no,
                )
                .await
                .map_err(|e| format!("Failed to find links: {}", e))?;

            let chosen = links
                .iter()
                .find(|l| l.raw_item_id == Some(chosen_raw_item_id))
                .map(|l| l.link_id);

            match chosen {
                Some(chosen_link_id) => {
                    self.resolve_conflict(conflict.conflict_id, chosen_link_id)
                        .await?;
                    resolved_ids.push(conflict.conflict_id);
                    chosen_links.push(chosen_link_id);
                    unchosen_links.extend(
                        links
                            .iter()
                            .filter(|l| l.link_id != chosen_link_id)
                            .map(|l| l.link_id),
                    );
                }
                None => {
                    skipped_ids.push(conflict.conflict_id);
                }
            }
        }

        Ok((resolved_ids, skipped_ids, chosen_links, unchosen_links))
    }

    /// Resolve a conflict: set chosen link as active (conflict_flag=false),
    /// mark others as resolved (link_status='resolved').
    pub async fn resolve_conflict(
        &self,
        conflict_id: i32,
        chosen_link_id: i32,
    ) -> Result<(), String> {
        // 1. Get the conflict record
        let conflict = self
            .conflict_repo
            .find_by_id(conflict_id)
            .await
            .map_err(|e| format!("Failed to find conflict: {}", e))?
            .ok_or_else(|| format!("Conflict {} not found", conflict_id))?;

        if conflict.resolution_status == "resolved" {
            return Err("Conflict already resolved".to_string());
        }

        // 2. Get all active links for this episode
        let links = self
            .link_repo
            .find_active_links_for_episode(
                conflict.anime_id,
                conflict.group_id,
                conflict.episode_no,
            )
            .await
            .map_err(|e| format!("Failed to find links: {}", e))?;

        // 3. Verify chosen_link_id is in the group
        if !links.iter().any(|l| l.link_id == chosen_link_id) {
            return Err(format!(
                "Link {} is not in the conflict group",
                chosen_link_id
            ));
        }

        // 4. Set chosen link: conflict_flag = false (stays active)
        self.link_repo
            .set_conflict_flags(&[chosen_link_id], false)
            .await
            .map_err(|e| format!("Failed to clear conflict flag: {}", e))?;

        // 5. Set others: link_status = 'resolved' (conflict_flag stays true)
        let other_ids: Vec<i32> = links
            .iter()
            .filter(|l| l.link_id != chosen_link_id)
            .map(|l| l.link_id)
            .collect();

        if !other_ids.is_empty() {
            self.link_repo
                .set_link_status(&other_ids, "resolved")
                .await
                .map_err(|e| format!("Failed to set link status: {}", e))?;
        }

        // 6. Update conflict record
        self.conflict_repo
            .resolve(conflict_id, chosen_link_id)
            .await
            .map_err(|e| format!("Failed to resolve conflict: {}", e))?;

        tracing::info!(
            "Resolved conflict {}: chosen link_id={}, {} others resolved",
            conflict_id,
            chosen_link_id,
            other_ids.len()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::anime_link::mock::MockAnimeLinkRepository;
    use crate::db::repository::anime_link_conflict::mock::MockAnimeLinkConflictRepository;
    use crate::models::AnimeLink;
    use chrono::Utc;
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::PgConnection;

    fn fake_pool() -> Arc<DbPool> {
        let manager = ConnectionManager::<PgConnection>::new("postgres://invalid/none");
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .min_idle(Some(0))
            .test_on_check_out(false)
            .build_unchecked(manager);
        Arc::new(pool)
    }

    fn link(link_id: i32, episode_no: i32, raw_item_id: Option<i32>) -> AnimeLink {
        AnimeLink {
            link_id,
            anime_id: 1,
            group_id: 1,
            episode_no,
            title: Some(format!("ep{}", episode_no)),
            url: format!("magnet:?xt=urn:btih:{}", link_id),
            source_hash: format!("hash{}", link_id),
            filtered_flag: false,
            created_at: Utc::now().naive_utc(),
            raw_item_id,
            download_type: Some("magnet".to_string()),
            conflict_flag: true,
            link_status: "active".to_string(),
        }
    }

    /// Batch raw_item 100 covers ep1+ep2; single raw_item 200 covers ep1.
    /// 衝突在 ep1。 Resolve by chosen_raw_item_id=100 → conflict 1 解決；
    /// chosen=link 1 (batch ep1)，link 3 (single ep1) 變 resolved。
    /// Ep2 沒有衝突（無第二個來源），故 conflict 表只有一筆，回應的 dispatched 為 [1]。
    #[tokio::test]
    async fn resolve_by_raw_item_picks_batch_link() {
        let link_repo = Arc::new(MockAnimeLinkRepository::with_data(vec![
            link(1, 1, Some(100)), // batch ep1
            link(2, 2, Some(100)), // batch ep2 (no conflict)
            link(3, 1, Some(200)), // single ep1
        ]));
        let conflict_repo = Arc::new(MockAnimeLinkConflictRepository::new());
        // create one conflict for ep1
        conflict_repo.upsert(1, 1, 1).await.unwrap();

        // Make ep2's link not in conflict
        link_repo.set_conflict_flags(&[2], false).await.unwrap();

        let svc =
            ConflictDetectionService::new(link_repo.clone(), conflict_repo.clone(), fake_pool());

        let (resolved, skipped, chosen, unchosen) =
            svc.resolve_conflicts_by_raw_item(1, 1, 100).await.unwrap();

        assert_eq!(resolved.len(), 1, "ep1 conflict should be resolved");
        assert!(skipped.is_empty(), "no conflict should be skipped");
        assert_eq!(chosen, vec![1], "should choose batch ep1 link");
        assert_eq!(unchosen, vec![3], "single ep1 link should be unchosen");
    }

    /// 當 chosen_raw_item 在某衝突無候選 link 時，該衝突應被 skip。
    #[tokio::test]
    async fn resolve_by_raw_item_skips_unmatched_conflicts() {
        let link_repo = Arc::new(MockAnimeLinkRepository::with_data(vec![
            link(1, 1, Some(100)),
            link(2, 1, Some(200)),
            // ep3 衝突不含 raw_item 100
            link(3, 3, Some(300)),
            link(4, 3, Some(400)),
        ]));
        let conflict_repo = Arc::new(MockAnimeLinkConflictRepository::new());
        conflict_repo.upsert(1, 1, 1).await.unwrap();
        let c3 = conflict_repo.upsert(1, 1, 3).await.unwrap();

        let svc =
            ConflictDetectionService::new(link_repo.clone(), conflict_repo.clone(), fake_pool());

        let (resolved, skipped, chosen, _) =
            svc.resolve_conflicts_by_raw_item(1, 1, 100).await.unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(skipped, vec![c3.conflict_id]);
        assert_eq!(chosen, vec![1]);
    }

    /// 不同 (anime_id, group_id) 的衝突不應被影響。
    #[tokio::test]
    async fn resolve_by_raw_item_scoped_to_anime_group() {
        let link_repo = Arc::new(MockAnimeLinkRepository::with_data(vec![
            link(1, 1, Some(100)),
            link(2, 1, Some(200)),
        ]));
        let conflict_repo = Arc::new(MockAnimeLinkConflictRepository::new());
        // 不同 anime_id 的衝突
        let other = conflict_repo.upsert(99, 99, 1).await.unwrap();

        let svc =
            ConflictDetectionService::new(link_repo.clone(), conflict_repo.clone(), fake_pool());

        let (resolved, skipped, _, _) = svc.resolve_conflicts_by_raw_item(1, 1, 100).await.unwrap();

        assert!(resolved.is_empty());
        assert!(skipped.is_empty());

        // verify the unrelated conflict still unresolved
        let still = conflict_repo.find_by_id(other.conflict_id).await.unwrap();
        assert_eq!(still.unwrap().resolution_status, "unresolved");
    }

    // ---------- 合集自動略過 ----------

    mod redundant_batches {
        use super::super::skip_redundant_batches;
        use crate::services::identity::test_db;
        use diesel::prelude::*;
        use diesel::sql_types::{Integer, Text};

        #[derive(QueryableByName)]
        struct Id {
            #[diesel(sql_type = Integer)]
            id: i32,
        }

        fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
            diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
        }

        struct Season {
            anime_id: i32,
            group_id: i32,
            sub_id: i32,
        }

        fn season(conn: &mut PgConnection) -> Season {
            let fetcher = insert(conn, "INSERT INTO service_modules (module_type, name, version, is_enabled, priority, base_url, created_at, updated_at) VALUES ('fetcher', 'batch-test-fetcher', '1', true, 1, 'http://f', now(), now()) RETURNING module_id AS id");
            let sub_id = insert(conn, &format!("INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) VALUES ({fetcher}, 'https://batch-test', 's', now(), now()) RETURNING subscription_id AS id"));
            let work = insert(conn, "INSERT INTO anime_works (title, created_at, updated_at) VALUES ('合集測試', now(), now()) RETURNING work_id AS id");
            let season_id = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2099, 'winter', now()) RETURNING season_id AS id");
            let anime_id = insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, 1, {season_id}, now(), now()) RETURNING anime_id AS id"));
            let group_id = insert(conn, "INSERT INTO subtitle_groups (group_name) VALUES ('合集測試組') RETURNING group_id AS id");
            Season {
                anime_id,
                group_id,
                sub_id,
            }
        }

        /// 一個 raw item 及其展開的 link；回傳 link id（依集數順序）
        fn release(
            conn: &mut PgConnection,
            s: &Season,
            name: &str,
            eps: std::ops::RangeInclusive<i32>,
        ) -> Vec<i32> {
            let batch = eps.start() != eps.end();
            let item = diesel::sql_query("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, created_at) VALUES ($1, $1, $2, 'parsed', now()) RETURNING item_id AS id")
                .bind::<Text, _>(name)
                .bind::<Integer, _>(s.sub_id)
                .get_result::<Id>(conn)
                .unwrap()
                .id;
            eps.map(|ep| {
                let hash = if batch { format!("{name}#ep{ep}") } else { name.to_string() };
                diesel::sql_query("INSERT INTO anime_links (anime_id, group_id, episode_no, url, source_hash, raw_item_id) VALUES ($1, $2, $3, $4, $5, $6) RETURNING link_id AS id")
                    .bind::<Integer, _>(s.anime_id)
                    .bind::<Integer, _>(s.group_id)
                    .bind::<Integer, _>(ep)
                    .bind::<Text, _>(name)
                    .bind::<Text, _>(hash)
                    .bind::<Integer, _>(item)
                    .get_result::<Id>(conn)
                    .unwrap()
                    .id
            })
            .collect()
        }

        fn download(conn: &mut PgConnection, link_id: i32, status: &str) {
            diesel::sql_query("INSERT INTO downloads (link_id, downloader_type, status) VALUES ($1, 'qbittorrent', $2)")
                .bind::<Integer, _>(link_id)
                .bind::<Text, _>(status)
                .execute(conn)
                .unwrap();
        }

        fn statuses(conn: &mut PgConnection, links: &[i32]) -> Vec<String> {
            use crate::schema::anime_links;
            links
                .iter()
                .map(|id| {
                    anime_links::table
                        .find(id)
                        .select(anime_links::link_status)
                        .first(conn)
                        .unwrap()
                })
                .collect()
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_batch_whose_episodes_are_all_downloaded_is_skipped() {
            let mut conn = test_db::conn();
            let s = season(&mut conn);
            for ep in 1..=3 {
                let single = release(&mut conn, &s, &format!("single-{ep}"), ep..=ep);
                download(
                    &mut conn,
                    single[0],
                    if ep == 1 { "synced" } else { "downloading" },
                );
            }
            let batch = release(&mut conn, &s, "batch", 1..=3);
            let skipped = skip_redundant_batches(&mut conn).unwrap();
            assert_eq!(skipped, batch);
            assert_eq!(statuses(&mut conn, &batch), vec!["resolved"; 3]);
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_batch_with_new_episodes_is_left_for_the_user() {
            let mut conn = test_db::conn();
            let s = season(&mut conn);
            for ep in 1..=2 {
                let single = release(&mut conn, &s, &format!("single-{ep}"), ep..=ep);
                download(&mut conn, single[0], "synced");
            }
            let batch = release(&mut conn, &s, "batch", 1..=3);
            assert!(skip_redundant_batches(&mut conn).unwrap().is_empty());
            assert_eq!(statuses(&mut conn, &batch), vec!["active"; 3]);
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn failed_or_cancelled_downloads_do_not_count() {
            let mut conn = test_db::conn();
            let s = season(&mut conn);
            let first = release(&mut conn, &s, "single-1", 1..=1);
            download(&mut conn, first[0], "failed");
            let second = release(&mut conn, &s, "single-2", 2..=2);
            download(&mut conn, second[0], "cancelled");
            release(&mut conn, &s, "batch", 1..=2);
            assert!(skip_redundant_batches(&mut conn).unwrap().is_empty());
        }

        #[test]
        #[ignore = "needs DATABASE_TEST_URL"]
        fn a_batch_already_being_downloaded_is_untouched() {
            let mut conn = test_db::conn();
            let s = season(&mut conn);
            for ep in 1..=2 {
                let single = release(&mut conn, &s, &format!("single-{ep}"), ep..=ep);
                download(&mut conn, single[0], "synced");
            }
            let batch = release(&mut conn, &s, "batch", 1..=2);
            download(&mut conn, batch[0], "downloading");
            assert!(skip_redundant_batches(&mut conn).unwrap().is_empty());
        }
    }
}
