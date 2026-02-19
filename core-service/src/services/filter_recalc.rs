//! Filter recalculation service
//!
//! When filter rules are created/deleted, this service recalculates the
//! `filtered_flag` on affected AnimeLinks.

use crate::db::DbPool;
use crate::models::{AnimeLink, FilterRule, FilterTargetType};
use crate::schema::{anime_links, anime_series, filter_rules, raw_anime_items};
use crate::services::filter::FilterEngine;
use diesel::prelude::*;

/// Result of recalculating filtered flags.
pub struct FilterRecalcResult {
    pub updated_count: usize,
    /// Link IDs that changed from unfiltered to filtered (false → true): need download cancellation.
    pub newly_filtered: Vec<i32>,
    /// Link IDs that changed from filtered to unfiltered (true → false): need download dispatch.
    pub newly_unfiltered: Vec<i32>,
}

/// Recalculate `filtered_flag` for AnimeLinks affected by a filter rule change.
pub fn recalculate_filtered_flags(
    conn: &mut PgConnection,
    target_type: FilterTargetType,
    target_id: Option<i32>,
) -> Result<FilterRecalcResult, String> {
    // 1. Find affected links
    let affected_links = find_affected_links(conn, target_type, target_id)?;

    if affected_links.is_empty() {
        return Ok(FilterRecalcResult {
            updated_count: 0,
            newly_filtered: vec![],
            newly_unfiltered: vec![],
        });
    }

    let mut updated = 0;
    let mut newly_filtered: Vec<i32> = Vec::new();
    let mut newly_unfiltered: Vec<i32> = Vec::new();

    for link in &affected_links {
        let rules = collect_all_rules_for_link(conn, link)?;
        let engine = FilterEngine::with_priority_sorted(rules);
        let title = link.title.as_deref().unwrap_or("");
        // filtered_flag = true means filtered OUT (should NOT be included)
        let should_include = engine.should_include(title);
        let new_flag = !should_include;

        if new_flag != link.filtered_flag {
            diesel::update(anime_links::table.filter(anime_links::link_id.eq(link.link_id)))
                .set(anime_links::filtered_flag.eq(new_flag))
                .execute(conn)
                .map_err(|e| format!("Failed to update filtered_flag for link {}: {}", link.link_id, e))?;
            updated += 1;
            if new_flag {
                // false → true: newly filtered OUT
                newly_filtered.push(link.link_id);
            } else {
                // true → false: newly unfiltered (eligible for download)
                newly_unfiltered.push(link.link_id);
            }
        }
    }

    tracing::info!(
        "filter_recalc: checked {} links, updated {} ({} newly filtered, {} newly unfiltered) for {:?}/{:?}",
        affected_links.len(),
        updated,
        newly_filtered.len(),
        newly_unfiltered.len(),
        target_type,
        target_id
    );

    Ok(FilterRecalcResult {
        updated_count: updated,
        newly_filtered,
        newly_unfiltered,
    })
}

/// Calculate filtered_flag for a single newly created AnimeLink.
/// Returns the computed flag value.
pub fn compute_filtered_flag_for_link(
    conn: &mut PgConnection,
    link: &AnimeLink,
) -> Result<bool, String> {
    let rules = collect_all_rules_for_link(conn, link)?;
    let engine = FilterEngine::with_priority_sorted(rules);
    let title = link.title.as_deref().unwrap_or("");
    let should_include = engine.should_include(title);
    // filtered_flag = true means filtered OUT
    Ok(!should_include)
}

/// Find all AnimeLinks affected by a rule change on the given target.
pub fn find_affected_links(
    conn: &mut PgConnection,
    target_type: FilterTargetType,
    target_id: Option<i32>,
) -> Result<Vec<AnimeLink>, String> {
    match target_type {
        FilterTargetType::Global => {
            // All links affected
            anime_links::table
                .load::<AnimeLink>(conn)
                .map_err(|e| format!("Failed to load all links: {}", e))
        }
        FilterTargetType::AnimeSeries => {
            // Links in this series
            let sid = target_id.ok_or("anime_series target requires target_id")?;
            anime_links::table
                .filter(anime_links::series_id.eq(sid))
                .load::<AnimeLink>(conn)
                .map_err(|e| format!("Failed to load links for series {}: {}", sid, e))
        }
        FilterTargetType::Anime => {
            // Links in all series of this anime
            let aid = target_id.ok_or("anime target requires target_id")?;
            let series_ids: Vec<i32> = anime_series::table
                .filter(anime_series::anime_id.eq(aid))
                .select(anime_series::series_id)
                .load(conn)
                .map_err(|e| format!("Failed to load series for anime {}: {}", aid, e))?;

            anime_links::table
                .filter(anime_links::series_id.eq_any(&series_ids))
                .load::<AnimeLink>(conn)
                .map_err(|e| format!("Failed to load links for anime {}: {}", aid, e))
        }
        FilterTargetType::SubtitleGroup => {
            // Links for this subtitle group
            let gid = target_id.ok_or("subtitle_group target requires target_id")?;
            anime_links::table
                .filter(anime_links::group_id.eq(gid))
                .load::<AnimeLink>(conn)
                .map_err(|e| format!("Failed to load links for group {}: {}", gid, e))
        }
        FilterTargetType::Fetcher | FilterTargetType::Subscription => {
            // Links from this fetcher/subscription (via raw_item_id → raw_anime_items → subscription)
            let fid = target_id.ok_or("fetcher/subscription target requires target_id")?;
            let raw_item_ids: Vec<i32> = raw_anime_items::table
                .filter(raw_anime_items::subscription_id.eq(fid))
                .select(raw_anime_items::item_id)
                .load(conn)
                .map_err(|e| format!("Failed to load raw items for subscription {}: {}", fid, e))?;

            anime_links::table
                .filter(anime_links::raw_item_id.eq_any(&raw_item_ids))
                .load::<AnimeLink>(conn)
                .map_err(|e| format!("Failed to load links for subscription {}: {}", fid, e))
        }
    }
}

/// Collect all applicable filter rules for a single AnimeLink.
///
/// Rules come from: global, anime (via series→anime), anime_series, subtitle_group,
/// and fetcher (via raw_item_id→raw_anime_items→subscription_id).
pub fn collect_all_rules_for_link(
    conn: &mut PgConnection,
    link: &AnimeLink,
) -> Result<Vec<FilterRule>, String> {
    // Get anime_id from the link's series
    let anime_id: i32 = anime_series::table
        .filter(anime_series::series_id.eq(link.series_id))
        .select(anime_series::anime_id)
        .first(conn)
        .map_err(|e| format!("Failed to get anime_id for series {}: {}", link.series_id, e))?;

    // Get fetcher/subscription_id from raw_item if available
    let subscription_id: Option<i32> = if let Some(raw_id) = link.raw_item_id {
        raw_anime_items::table
            .filter(raw_anime_items::item_id.eq(raw_id))
            .select(raw_anime_items::subscription_id)
            .first(conn)
            .optional()
            .map_err(|e| format!("Failed to get subscription for raw_item {}: {}", raw_id, e))?
    } else {
        None
    };

    // Load all applicable rules in one query using OR conditions
    let mut all_rules: Vec<FilterRule> = Vec::new();

    // Global rules
    let global_rules: Vec<FilterRule> = filter_rules::table
        .filter(filter_rules::target_type.eq(FilterTargetType::Global))
        .filter(filter_rules::target_id.is_null())
        .load(conn)
        .map_err(|e| format!("Failed to load global rules: {}", e))?;
    all_rules.extend(global_rules);

    // Anime rules
    let anime_rules: Vec<FilterRule> = filter_rules::table
        .filter(filter_rules::target_type.eq(FilterTargetType::Anime))
        .filter(filter_rules::target_id.eq(anime_id))
        .load(conn)
        .map_err(|e| format!("Failed to load anime rules: {}", e))?;
    all_rules.extend(anime_rules);

    // AnimeSeries rules
    let series_rules: Vec<FilterRule> = filter_rules::table
        .filter(filter_rules::target_type.eq(FilterTargetType::AnimeSeries))
        .filter(filter_rules::target_id.eq(link.series_id))
        .load(conn)
        .map_err(|e| format!("Failed to load series rules: {}", e))?;
    all_rules.extend(series_rules);

    // SubtitleGroup rules
    let group_rules: Vec<FilterRule> = filter_rules::table
        .filter(filter_rules::target_type.eq(FilterTargetType::SubtitleGroup))
        .filter(filter_rules::target_id.eq(link.group_id))
        .load(conn)
        .map_err(|e| format!("Failed to load group rules: {}", e))?;
    all_rules.extend(group_rules);

    // Fetcher rules (if subscription known)
    if let Some(sub_id) = subscription_id {
        let fetcher_rules: Vec<FilterRule> = filter_rules::table
            .filter(filter_rules::target_type.eq(FilterTargetType::Fetcher))
            .filter(filter_rules::target_id.eq(sub_id))
            .load(conn)
            .map_err(|e| format!("Failed to load fetcher rules: {}", e))?;
        all_rules.extend(fetcher_rules);
    }

    Ok(all_rules)
}
