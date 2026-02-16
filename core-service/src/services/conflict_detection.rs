use std::sync::Arc;

use crate::db::repository::{AnimeLinkConflictRepository, AnimeLinkRepository};

pub struct ConflictDetectionService {
    link_repo: Arc<dyn AnimeLinkRepository>,
    conflict_repo: Arc<dyn AnimeLinkConflictRepository>,
}

impl ConflictDetectionService {
    pub fn new(
        link_repo: Arc<dyn AnimeLinkRepository>,
        conflict_repo: Arc<dyn AnimeLinkConflictRepository>,
    ) -> Self {
        Self {
            link_repo,
            conflict_repo,
        }
    }

    /// Scan all active unfiltered anime_links, detect conflicts (same series_id, group_id,
    /// episode_no with COUNT > 1), set conflict_flag, and upsert anime_link_conflicts records.
    /// Also clears conflict_flag for groups that are no longer conflicting.
    pub async fn detect_and_mark_conflicts(&self) -> Result<usize, String> {
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

        for (series_id, group_id, episode_no) in &conflict_groups {
            let links = self
                .link_repo
                .find_active_links_for_episode(*series_id, *group_id, *episode_no)
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
                    .upsert(*series_id, *group_id, *episode_no)
                    .await
                    .map_err(|e| format!("Failed to upsert conflict: {}", e))?;

                conflicts_found += 1;

                tracing::info!(
                    "Conflict detected: series_id={}, group_id={}, episode_no={}, {} links",
                    series_id,
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
        for conflict in &unresolved {
            let links = self
                .link_repo
                .find_active_links_for_episode(
                    conflict.series_id,
                    conflict.group_id,
                    conflict.episode_no,
                )
                .await
                .map_err(|e| format!("Failed to check conflict links: {}", e))?;

            if links.len() <= 1 {
                // No longer a conflict — delete the conflict record
                let _ = self
                    .conflict_repo
                    .delete_by_episode(
                        conflict.series_id,
                        conflict.group_id,
                        conflict.episode_no,
                    )
                    .await;
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

        Ok(conflicts_found)
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
                conflict.series_id,
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
