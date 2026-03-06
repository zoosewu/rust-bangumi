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
        // 收集各訂閱的衝突群組，用於批次 AI filter 生成
        // key: Option<subscription_id>, value: Vec<(titles, source_title)>
        let mut subscription_conflict_groups: std::collections::HashMap<
            Option<i32>,
            Vec<(Vec<String>, String)>,
        > = std::collections::HashMap::new();

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

                // 收集衝突標題，準備批次 AI filter 生成
                let conflict_titles: Vec<String> = links
                    .iter()
                    .filter_map(|l| l.title.clone())
                    .collect();
                if !conflict_titles.is_empty() {
                    let source = conflict_titles[0].clone();

                    // 從 links 的 raw_item_id 查詢 subscription_id
                    let filter_sub_id: Option<i32> = {
                        use crate::schema::raw_anime_items;
                        let raw_id = links.iter().find_map(|l| l.raw_item_id);
                        raw_id.and_then(|rid| {
                            let mut conn = self.pool.get().ok()?;
                            raw_anime_items::table
                                .filter(raw_anime_items::item_id.eq(rid))
                                .select(raw_anime_items::subscription_id)
                                .first::<i32>(&mut conn)
                                .ok()
                        })
                    };

                    subscription_conflict_groups
                        .entry(filter_sub_id)
                        .or_default()
                        .push((conflict_titles, source));
                }
            }
        }

        // 每個訂閱批次觸發一次 AI filter 生成（背景非同步）
        for (sub_id, groups) in subscription_conflict_groups {
            let pool_clone = self.pool.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    crate::ai::filter_generator::generate_filters_for_subscription_batch(
                        pool_clone, sub_id, groups,
                    )
                    .await
                {
                    tracing::warn!("批次 AI filter 觸發失敗 subscription={:?}: {}", sub_id, e);
                }
            });
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
