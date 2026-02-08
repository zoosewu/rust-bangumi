use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;

// ============ BangumiSubject ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::bangumi_subjects)]
pub struct BangumiSubject {
    pub bangumi_id: i32,
    pub title: String,
    pub title_cn: Option<String>,
    pub summary: Option<String>,
    pub rating: Option<f32>,
    pub cover_url: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub episode_count: Option<i32>,
    pub raw_json: Option<serde_json::Value>,
    pub fetched_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_subjects)]
pub struct NewBangumiSubject {
    pub bangumi_id: i32,
    pub title: String,
    pub title_cn: Option<String>,
    pub summary: Option<String>,
    pub rating: Option<f32>,
    pub cover_url: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub episode_count: Option<i32>,
    pub raw_json: Option<serde_json::Value>,
}

// ============ BangumiEpisode ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::bangumi_episodes)]
pub struct BangumiEpisode {
    pub bangumi_ep_id: i32,
    pub bangumi_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub summary: Option<String>,
    pub fetched_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_episodes)]
pub struct NewBangumiEpisode {
    pub bangumi_ep_id: i32,
    pub bangumi_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub summary: Option<String>,
}

// ============ BangumiMapping ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::bangumi_mapping)]
pub struct BangumiMapping {
    pub core_series_id: i32,
    pub bangumi_id: i32,
    pub title_cache: Option<String>,
    pub source: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_mapping)]
pub struct NewBangumiMapping {
    pub core_series_id: i32,
    pub bangumi_id: i32,
    pub title_cache: Option<String>,
    pub source: String,
}

// ============ SyncTask ============

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::sync_tasks)]
pub struct SyncTask {
    pub task_id: i32,
    pub download_id: i32,
    pub core_series_id: i32,
    pub episode_no: i32,
    pub source_path: String,
    pub target_path: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::sync_tasks)]
pub struct NewSyncTask {
    pub download_id: i32,
    pub core_series_id: i32,
    pub episode_no: i32,
    pub source_path: String,
    pub status: String,
}
