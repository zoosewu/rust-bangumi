use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;
use serde_json::Value as JsonValue;
use std::io::Write;

// ============ FilterTargetType ENUM ============
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    diesel::deserialize::FromSqlRow,
    diesel::expression::AsExpression,
)]
#[diesel(sql_type = crate::schema::sql_types::FilterTargetType)]
pub enum FilterTargetType {
    Global,
    Anime,
    SubtitleGroup,
    AnimeSeries,
    Fetcher,
    Subscription,
}

impl diesel::deserialize::FromSql<crate::schema::sql_types::FilterTargetType, diesel::pg::Pg>
    for FilterTargetType
{
    fn from_sql(bytes: diesel::pg::PgValue<'_>) -> diesel::deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"global" => Ok(FilterTargetType::Global),
            b"anime" => Ok(FilterTargetType::Anime),
            b"subtitle_group" => Ok(FilterTargetType::SubtitleGroup),
            b"anime_series" => Ok(FilterTargetType::AnimeSeries),
            b"fetcher" => Ok(FilterTargetType::Fetcher),
            b"subscription" => Ok(FilterTargetType::Subscription),
            _ => Err("Unrecognized filter_target_type variant".into()),
        }
    }
}

impl diesel::serialize::ToSql<crate::schema::sql_types::FilterTargetType, diesel::pg::Pg>
    for FilterTargetType
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, diesel::pg::Pg>,
    ) -> diesel::serialize::Result {
        match *self {
            FilterTargetType::Global => out.write_all(b"global")?,
            FilterTargetType::Anime => out.write_all(b"anime")?,
            FilterTargetType::SubtitleGroup => out.write_all(b"subtitle_group")?,
            FilterTargetType::AnimeSeries => out.write_all(b"anime_series")?,
            FilterTargetType::Fetcher => out.write_all(b"fetcher")?,
            FilterTargetType::Subscription => out.write_all(b"subscription")?,
        }
        Ok(diesel::serialize::IsNull::No)
    }
}

impl std::fmt::Display for FilterTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterTargetType::Global => write!(f, "global"),
            FilterTargetType::Anime => write!(f, "anime"),
            FilterTargetType::SubtitleGroup => write!(f, "subtitle_group"),
            FilterTargetType::AnimeSeries => write!(f, "anime_series"),
            FilterTargetType::Fetcher => write!(f, "fetcher"),
            FilterTargetType::Subscription => write!(f, "subscription"),
        }
    }
}

impl std::str::FromStr for FilterTargetType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "global" => Ok(FilterTargetType::Global),
            "anime" => Ok(FilterTargetType::Anime),
            "subtitle_group" => Ok(FilterTargetType::SubtitleGroup),
            "anime_series" => Ok(FilterTargetType::AnimeSeries),
            "fetcher" => Ok(FilterTargetType::Fetcher),
            "subscription" => Ok(FilterTargetType::Subscription),
            _ => Err(format!("Unknown filter target type: {}", s)),
        }
    }
}

// ============ ModuleType ENUM ============
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    diesel::deserialize::FromSqlRow,
    diesel::expression::AsExpression,
)]
#[diesel(sql_type = crate::schema::sql_types::ModuleType)]
pub enum ModuleTypeEnum {
    Fetcher,
    Downloader,
    Viewer,
}

impl diesel::deserialize::FromSql<crate::schema::sql_types::ModuleType, diesel::pg::Pg>
    for ModuleTypeEnum
{
    fn from_sql(bytes: diesel::pg::PgValue<'_>) -> diesel::deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"fetcher" => Ok(ModuleTypeEnum::Fetcher),
            b"downloader" => Ok(ModuleTypeEnum::Downloader),
            b"viewer" => Ok(ModuleTypeEnum::Viewer),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl diesel::serialize::ToSql<crate::schema::sql_types::ModuleType, diesel::pg::Pg>
    for ModuleTypeEnum
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, diesel::pg::Pg>,
    ) -> diesel::serialize::Result {
        match *self {
            ModuleTypeEnum::Fetcher => out.write_all(b"fetcher")?,
            ModuleTypeEnum::Downloader => out.write_all(b"downloader")?,
            ModuleTypeEnum::Viewer => out.write_all(b"viewer")?,
        }
        Ok(diesel::serialize::IsNull::No)
    }
}

impl std::fmt::Display for ModuleTypeEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleTypeEnum::Fetcher => write!(f, "fetcher"),
            ModuleTypeEnum::Downloader => write!(f, "downloader"),
            ModuleTypeEnum::Viewer => write!(f, "viewer"),
        }
    }
}

impl From<&shared::ServiceType> for ModuleTypeEnum {
    fn from(service_type: &shared::ServiceType) -> Self {
        match service_type {
            shared::ServiceType::Fetcher => ModuleTypeEnum::Fetcher,
            shared::ServiceType::Downloader => ModuleTypeEnum::Downloader,
            shared::ServiceType::Viewer => ModuleTypeEnum::Viewer,
        }
    }
}

// ============ Seasons ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::seasons)]
pub struct Season {
    pub season_id: i32,
    pub year: i32,
    pub season: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::seasons)]
pub struct NewSeason {
    pub year: i32,
    pub season: String,
    pub created_at: NaiveDateTime,
}

// ============ Animes ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::animes)]
pub struct Anime {
    pub anime_id: i32,
    pub title: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::animes)]
pub struct NewAnime {
    pub title: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ AnimeSeries ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::anime_series)]
pub struct AnimeSeries {
    pub series_id: i32,
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::anime_series)]
pub struct NewAnimeSeries {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ SubtitleGroups ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::subtitle_groups)]
pub struct SubtitleGroup {
    pub group_id: i32,
    pub group_name: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::subtitle_groups)]
pub struct NewSubtitleGroup {
    pub group_name: String,
    pub created_at: NaiveDateTime,
}

// ============ AnimeLinks ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::anime_links)]
pub struct AnimeLink {
    pub link_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: NaiveDateTime,
    pub raw_item_id: Option<i32>,
    pub download_type: Option<String>,
    pub conflict_flag: bool,
    pub link_status: String,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::anime_links)]
pub struct NewAnimeLink {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub title: Option<String>,
    pub url: String,
    pub source_hash: String,
    pub filtered_flag: bool,
    pub created_at: NaiveDateTime,
    pub raw_item_id: Option<i32>,
    pub download_type: Option<String>,
    pub conflict_flag: bool,
    pub link_status: String,
}

// ============ AnimeLinkConflicts ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::anime_link_conflicts)]
pub struct AnimeLinkConflict {
    pub conflict_id: i32,
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub resolution_status: String,
    pub chosen_link_id: Option<i32>,
    pub created_at: NaiveDateTime,
    pub resolved_at: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::anime_link_conflicts)]
pub struct NewAnimeLinkConflict {
    pub series_id: i32,
    pub group_id: i32,
    pub episode_no: i32,
    pub resolution_status: String,
    pub created_at: NaiveDateTime,
}

// ============ FilterRules ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::filter_rules)]
pub struct FilterRule {
    pub rule_id: i32,
    pub rule_order: i32,
    pub regex_pattern: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub is_positive: bool,
    pub target_type: FilterTargetType,
    pub target_id: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::filter_rules)]
pub struct NewFilterRule {
    pub rule_order: i32,
    pub regex_pattern: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub is_positive: bool,
    pub target_type: FilterTargetType,
    pub target_id: Option<i32>,
}

// ============ Downloads ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct Download {
    pub download_id: i32,
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub module_id: Option<i32>,
    pub torrent_hash: Option<String>,
    pub file_path: Option<String>,
    pub sync_retry_count: i32,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::downloads)]
pub struct NewDownload {
    pub link_id: i32,
    pub downloader_type: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub module_id: Option<i32>,
    pub torrent_hash: Option<String>,
}

// ============ DownloaderCapabilities ============
#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::downloader_capabilities)]
pub struct DownloaderCapability {
    pub module_id: i32,
    pub download_type: String,
}

// ============ CronLogs ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::cron_logs)]
pub struct CronLog {
    pub log_id: i32,
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
    pub executed_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::cron_logs)]
pub struct NewCronLog {
    pub fetcher_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub attempt_count: i32,
    pub executed_at: NaiveDateTime,
}

// ============ ServiceModules ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::service_modules)]
pub struct ServiceModule {
    pub module_id: i32,
    pub module_type: ModuleTypeEnum,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub priority: i32,
    pub base_url: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::service_modules)]
pub struct NewServiceModule {
    pub module_type: ModuleTypeEnum,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub priority: i32,
    pub base_url: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ Subscriptions (formerly RssSubscriptions) ============
#[derive(Queryable, Selectable, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = super::super::schema::subscriptions)]
pub struct Subscription {
    pub subscription_id: i32,
    pub fetcher_id: i32,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub next_fetch_at: Option<NaiveDateTime>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub config: Option<JsonValue>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub source_type: String,
    pub assignment_status: String,
    pub assigned_at: Option<NaiveDateTime>,
    pub auto_selected: bool,
}

// For manual inserts, use sql_query with bind parameters instead
#[derive(Insertable)]
#[diesel(table_name = super::super::schema::subscriptions)]
pub struct NewSubscription {
    pub fetcher_id: i32,
    pub source_url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub next_fetch_at: Option<NaiveDateTime>,
    pub fetch_interval_minutes: i32,
    pub is_active: bool,
    pub config: Option<JsonValue>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub source_type: String,
    pub assignment_status: String,
    pub assigned_at: Option<NaiveDateTime>,
    pub auto_selected: bool,
}

// Compatibility alias for existing code
pub type RssSubscription = Subscription;
pub type NewRssSubscription = NewSubscription;

// ============ SubscriptionConflicts ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::subscription_conflicts)]
pub struct SubscriptionConflict {
    pub conflict_id: i32,
    pub subscription_id: i32,
    pub conflict_type: String,
    pub affected_item_id: Option<String>,
    pub conflict_data: JsonValue,
    pub resolution_status: String,
    pub resolution_data: Option<JsonValue>,
    pub created_at: NaiveDateTime,
    pub resolved_at: Option<NaiveDateTime>,
}

// For manual inserts, use sql_query with bind parameters instead
#[derive(Insertable)]
#[diesel(table_name = super::super::schema::subscription_conflicts)]
pub struct NewSubscriptionConflict {
    pub subscription_id: i32,
    pub conflict_type: String,
    pub affected_item_id: Option<String>,
    pub conflict_data: JsonValue,
    pub resolution_status: String,
    pub resolution_data: Option<JsonValue>,
    pub created_at: NaiveDateTime,
    pub resolved_at: Option<NaiveDateTime>,
}

// ============ ParserSourceType ENUM ============
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    diesel::deserialize::FromSqlRow,
    diesel::expression::AsExpression,
)]
#[diesel(sql_type = crate::schema::sql_types::ParserSourceType)]
pub enum ParserSourceType {
    Regex,
    Static,
}

impl diesel::deserialize::FromSql<crate::schema::sql_types::ParserSourceType, diesel::pg::Pg>
    for ParserSourceType
{
    fn from_sql(bytes: diesel::pg::PgValue<'_>) -> diesel::deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"regex" => Ok(ParserSourceType::Regex),
            b"static" => Ok(ParserSourceType::Static),
            _ => Err("Unrecognized parser_source_type variant".into()),
        }
    }
}

impl diesel::serialize::ToSql<crate::schema::sql_types::ParserSourceType, diesel::pg::Pg>
    for ParserSourceType
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, diesel::pg::Pg>,
    ) -> diesel::serialize::Result {
        match *self {
            ParserSourceType::Regex => out.write_all(b"regex")?,
            ParserSourceType::Static => out.write_all(b"static")?,
        }
        Ok(diesel::serialize::IsNull::No)
    }
}

impl std::fmt::Display for ParserSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserSourceType::Regex => write!(f, "regex"),
            ParserSourceType::Static => write!(f, "static"),
        }
    }
}

// ============ TitleParsers ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::title_parsers)]
pub struct TitleParser {
    pub parser_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: bool,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: ParserSourceType,
    pub anime_title_value: String,
    pub episode_no_source: ParserSourceType,
    pub episode_no_value: String,
    pub series_no_source: Option<ParserSourceType>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<ParserSourceType>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<ParserSourceType>,
    pub resolution_value: Option<String>,
    pub season_source: Option<ParserSourceType>,
    pub season_value: Option<String>,
    pub year_source: Option<ParserSourceType>,
    pub year_value: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub created_from_type: Option<FilterTargetType>,
    pub created_from_id: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::title_parsers)]
pub struct NewTitleParser {
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub is_enabled: bool,
    pub condition_regex: String,
    pub parse_regex: String,
    pub anime_title_source: ParserSourceType,
    pub anime_title_value: String,
    pub episode_no_source: ParserSourceType,
    pub episode_no_value: String,
    pub series_no_source: Option<ParserSourceType>,
    pub series_no_value: Option<String>,
    pub subtitle_group_source: Option<ParserSourceType>,
    pub subtitle_group_value: Option<String>,
    pub resolution_source: Option<ParserSourceType>,
    pub resolution_value: Option<String>,
    pub season_source: Option<ParserSourceType>,
    pub season_value: Option<String>,
    pub year_source: Option<ParserSourceType>,
    pub year_value: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub created_from_type: Option<FilterTargetType>,
    pub created_from_id: Option<i32>,
}

// ============ RawAnimeItems ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::raw_anime_items)]
pub struct RawAnimeItem {
    pub item_id: i32,
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<NaiveDateTime>,
    pub subscription_id: i32,
    pub status: String,
    pub parser_id: Option<i32>,
    pub error_message: Option<String>,
    pub parsed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::raw_anime_items)]
pub struct NewRawAnimeItem {
    pub title: String,
    pub description: Option<String>,
    pub download_url: String,
    pub pub_date: Option<NaiveDateTime>,
    pub subscription_id: i32,
    pub status: String,
    pub parser_id: Option<i32>,
    pub error_message: Option<String>,
    pub parsed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}
