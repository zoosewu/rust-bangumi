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
                    .delete_by_episode(
                        conflict.anime_id,
                        conflict.group_id,
                        conflict.episode_no,
                    )
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

        let (resolved, skipped, _, _) =
            svc.resolve_conflicts_by_raw_item(1, 1, 100).await.unwrap();

        assert!(resolved.is_empty());
        assert!(skipped.is_empty());

        // verify the unrelated conflict still unresolved
        let still = conflict_repo.find_by_id(other.conflict_id).await.unwrap();
        assert_eq!(still.unwrap().resolution_status, "unresolved");
    }
}
