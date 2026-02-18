use chrono::NaiveDate;
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
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::bangumi_mapping)]
pub struct NewBangumiMapping {
    pub core_series_id: i32,
    pub bangumi_id: i32,
}

// ============ SyncTask ============

#[derive(Insertable)]
#[diesel(table_name = crate::schema::sync_tasks)]
pub struct NewSyncTask {
    pub download_id: i32,
    pub status: String,
}
