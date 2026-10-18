//! 作品身分：依 fetcher 回報的 external id 決定季與系列
//! （docs/superpowers/specs/2026-07-15-metadata-identity-design.md）。

use async_trait::async_trait;
use chrono::{Datelike, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use serde::Deserialize;
use shared::ExternalId;
use std::collections::{HashMap, HashSet};

use crate::db::DbPool;
use crate::models::db::{
    Anime, AnimeWork, MetadataNamespace, ModuleTypeEnum, NewAnime, NewAnimeCoverImage,
    NewAnimeExternalId, NewAnimeWork,
};
use crate::schema::{
    anime_cover_images, anime_external_ids, anime_works, animes, metadata_namespaces, seasons,
    service_modules,
};

/// metadata 模組註冊時，以這次宣告的 namespace 取代它先前的宣告。
pub fn save_metadata_namespaces(
    conn: &mut PgConnection,
    module_name: &str,
    namespaces: &[String],
) -> QueryResult<()> {
    conn.transaction(|conn| {
        let module_id: i32 = service_modules::table
            .filter(service_modules::name.eq(module_name))
            .select(service_modules::module_id)
            .first(conn)?;
        diesel::delete(
            metadata_namespaces::table.filter(metadata_namespaces::module_id.eq(module_id)),
        )
        .execute(conn)?;
        let rows: Vec<MetadataNamespace> = namespaces
            .iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .map(|namespace| MetadataNamespace {
                module_id,
                namespace: namespace.clone(),
                priority: 0,
            })
            .collect();
        diesel::insert_into(metadata_namespaces::table)
            .values(&rows)
            .execute(conn)?;
        Ok(())
    })
}

/// 目前啟用中的 metadata service 能解析的 namespace。
pub fn registered_namespaces(conn: &mut PgConnection) -> QueryResult<HashSet<String>> {
    metadata_namespaces::table
        .inner_join(service_modules::table)
        .filter(service_modules::is_enabled.eq(true))
        .filter(service_modules::module_type.eq(ModuleTypeEnum::Metadata))
        .select(metadata_namespaces::namespace)
        .distinct()
        .load::<String>(conn)
        .map(|namespaces| namespaces.into_iter().collect())
}

/// fetcher 回報的身分中，有 metadata service 能解析的那些。
pub fn usable_ids(ids: &[ExternalId], registered: &HashSet<String>) -> Vec<ExternalId> {
    ids.iter()
        .filter(|id| registered.contains(&id.namespace))
        .cloned()
        .collect()
}

// ---------- metadata service 查詢 ----------

#[derive(Debug, Clone, Deserialize)]
pub struct CoverImage {
    pub url: String,
    pub source: String,
}

/// 系列（第一季）的內容；`title` 是各季共用的系列名稱
#[derive(Debug, Clone, Deserialize)]
pub struct SeriesMeta {
    pub namespace: String,
    pub external_id: String,
    pub title: String,
    pub original_title: String,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    #[serde(default)]
    pub cover_images: Vec<CoverImage>,
}

/// metadata service `/enrich/anime` 回傳的一季內容
#[derive(Debug, Clone, Deserialize)]
pub struct SeasonMeta {
    pub namespace: String,
    pub external_id: String,
    pub title: String,
    pub title_cn: String,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(default)]
    pub cover_images: Vec<CoverImage>,
    pub series: SeriesMeta,
}

#[derive(Debug, Clone)]
pub struct FetchedMeta {
    pub meta: SeasonMeta,
    /// 提供這份資料的 metadata 模組（封面記錄來源用）
    pub module_id: Option<i32>,
}

#[async_trait]
pub trait MetadataLookup: Send + Sync {
    /// `Ok(None)`：沒有服務能解析這個 namespace，或 subject 不存在。
    async fn enrich_anime(&self, id: &ExternalId) -> Result<Option<FetchedMeta>, String>;
}

/// 向宣告了該 namespace 的 metadata service 查詢。
pub struct HttpMetadataLookup {
    db: DbPool,
    http: reqwest::Client,
}

impl HttpMetadataLookup {
    pub fn new(db: DbPool) -> Self {
        Self {
            db,
            // 前傳鏈要連續查幾次 bgm，給寬一點
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl MetadataLookup for HttpMetadataLookup {
    async fn enrich_anime(&self, id: &ExternalId) -> Result<Option<FetchedMeta>, String> {
        let service: Option<(i32, String)> = {
            let mut conn = self.db.get().map_err(|e| e.to_string())?;
            metadata_namespaces::table
                .inner_join(service_modules::table)
                .filter(metadata_namespaces::namespace.eq(&id.namespace))
                .filter(service_modules::is_enabled.eq(true))
                .filter(service_modules::module_type.eq(ModuleTypeEnum::Metadata))
                .order(metadata_namespaces::priority.desc())
                .select((service_modules::module_id, service_modules::base_url))
                .first(&mut conn)
                .optional()
                .map_err(|e| e.to_string())?
        };
        let Some((module_id, base_url)) = service else {
            return Ok(None);
        };
        let resp = self
            .http
            .post(format!("{}/enrich/anime", base_url))
            .json(&serde_json::json!({ "namespace": id.namespace, "external_id": id.id }))
            .send()
            .await
            .map_err(|e| format!("metadata service unreachable: {}", e))?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("metadata service returned HTTP {}", resp.status()));
        }
        resp.json::<SeasonMeta>()
            .await
            .map(|meta| {
                Some(FetchedMeta {
                    meta,
                    module_id: Some(module_id),
                })
            })
            .map_err(|e| format!("invalid metadata response: {}", e))
    }
}

// ---------- 季與系列的解析 ----------

#[derive(Debug)]
pub enum SeasonResolution {
    /// 身分決定的季
    Resolved(Anime),
    /// 沒有可用的身分 → 呼叫端沿用 parser 作品名（並寫 log）
    NoIdentity,
    /// parser 的季數已被另一個 subject 佔用 → 呼叫端把 raw item 標為 failed
    Conflict(String),
}

/// 一批 raw item 解析前先準備好的身分資料：哪些 namespace 可用，以及還不在資料庫裡的季的 metadata。
/// 網路查詢只在這裡做，之後的解析都是同步的資料庫操作。
pub struct IdentityContext {
    registered: HashSet<String>,
    metas: HashMap<ExternalId, FetchedMeta>,
}

impl IdentityContext {
    /// 不做任何查詢的空 context：所有 item 都走 parser 作品名。
    pub fn empty() -> Self {
        Self {
            registered: HashSet::new(),
            metas: HashMap::new(),
        }
    }

    pub async fn prepare(
        conn: &mut PgConnection,
        lookup: &dyn MetadataLookup,
        items: &[Vec<ExternalId>],
    ) -> Result<Self, String> {
        let registered = registered_namespaces(conn).map_err(|e| e.to_string())?;
        let mut metas = HashMap::new();
        let mut looked_up = HashSet::new();
        for id in items.iter().flat_map(|ids| usable_ids(ids, &registered)) {
            if !looked_up.insert(id.clone())
                || find_season(conn, &id).map_err(|e| e.to_string())?.is_some()
            {
                continue;
            }
            match lookup.enrich_anime(&id).await {
                Ok(Some(fetched)) => {
                    metas.insert(id, fetched);
                }
                Ok(None) => tracing::warn!("No metadata found for {}", id),
                Err(e) => tracing::warn!("Metadata lookup for {} failed: {}", id, e),
            }
        }
        Ok(Self { registered, metas })
    }
}

/// 依身分決定 raw item 屬於哪一季；需要時建立系列與季。
pub fn resolve_season(
    conn: &mut PgConnection,
    context: &IdentityContext,
    ids: &[ExternalId],
    series_no: i32,
) -> Result<SeasonResolution, String> {
    for id in usable_ids(ids, &context.registered) {
        if let Some(anime) = find_season(conn, &id).map_err(|e| e.to_string())? {
            return Ok(SeasonResolution::Resolved(anime));
        }
        let Some(fetched) = context.metas.get(&id) else {
            continue;
        };
        return match conn.transaction(|conn| create_season(conn, &id, series_no, fetched)) {
            Ok(resolution) => Ok(resolution),
            // 同時有另一個請求先建好了這一季
            Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                find_season(conn, &id)
                    .map_err(|e| e.to_string())?
                    .map(SeasonResolution::Resolved)
                    .ok_or_else(|| format!("season for {} vanished after a conflict", id))
            }
            Err(e) => Err(format!("Failed to create season for {}: {}", id, e)),
        };
    }
    Ok(SeasonResolution::NoIdentity)
}

fn find_season(conn: &mut PgConnection, id: &ExternalId) -> QueryResult<Option<Anime>> {
    animes::table
        .inner_join(anime_external_ids::table)
        .filter(anime_external_ids::namespace.eq(&id.namespace))
        .filter(anime_external_ids::external_id.eq(&id.id))
        .select(Anime::as_select())
        .first(conn)
        .optional()
}

/// 這一季在某個 namespace 的 external id。
pub fn find_external_id(
    conn: &mut PgConnection,
    anime_id: i32,
    namespace: &str,
) -> QueryResult<Option<String>> {
    anime_external_ids::table
        .filter(anime_external_ids::anime_id.eq(anime_id))
        .filter(anime_external_ids::namespace.eq(namespace))
        .select(anime_external_ids::external_id)
        .first(conn)
        .optional()
}

fn parse_date(value: Option<&str>) -> Option<NaiveDate> {
    value.and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
}

/// 播出日期所屬的年份與季度
pub fn season_of(date: NaiveDate) -> (i32, &'static str) {
    let quarter = match date.month() {
        1..=3 => "winter",
        4..=6 => "spring",
        7..=9 => "summer",
        _ => "fall",
    };
    (date.year(), quarter)
}

fn season_id_for(conn: &mut PgConnection, aired: Option<NaiveDate>) -> QueryResult<i32> {
    let (year, quarter) = aired
        .map(season_of)
        .unwrap_or((Utc::now().year(), "unknown"));
    let existing: Option<i32> = seasons::table
        .filter(seasons::year.eq(year))
        .filter(seasons::season.eq(quarter))
        .select(seasons::season_id)
        .first(conn)
        .optional()?;
    match existing {
        Some(id) => Ok(id),
        None => diesel::insert_into(seasons::table)
            .values((
                seasons::year.eq(year),
                seasons::season.eq(quarter),
                seasons::created_at.eq(Utc::now().naive_utc()),
            ))
            .returning(seasons::season_id)
            .get_result(conn),
    }
}

fn create_season(
    conn: &mut PgConnection,
    id: &ExternalId,
    series_no: i32,
    fetched: &FetchedMeta,
) -> QueryResult<SeasonResolution> {
    let meta = &fetched.meta;
    let work = find_or_create_series(conn, &meta.series)?;
    let aired = parse_date(meta.air_date.as_deref());
    let season_id = season_id_for(conn, aired)?;
    let now = Utc::now().naive_utc();

    let existing: Option<Anime> = animes::table
        .filter(animes::work_id.eq(work.work_id))
        .filter(animes::series_no.eq(series_no))
        .first(conn)
        .optional()?;
    let anime_id = match existing {
        Some(anime) => {
            if let Some(other) = find_external_id(conn, anime.anime_id, &id.namespace)? {
                return Ok(SeasonResolution::Conflict(format!(
                    "{} 第 {} 季已綁定 {}/{}，與 {} 不符；請確認 parser 解析的季數",
                    work.title, series_no, id.namespace, other, id
                )));
            }
            anime.anime_id
        }
        None => diesel::insert_into(animes::table)
            .values(NewAnime {
                work_id: work.work_id,
                series_no,
                season_id,
                description: None,
                aired_date: None,
                end_date: None,
                created_at: now,
                updated_at: now,
            })
            .returning(animes::anime_id)
            .get_result(conn)?,
    };

    let season_title = Some(meta.title_cn.trim())
        .filter(|t| !t.is_empty())
        .unwrap_or(meta.title.trim());
    let anime: Anime = diesel::update(animes::table.find(anime_id))
        .set((
            animes::title.eq(season_title),
            animes::original_title.eq(&meta.title),
            animes::description.eq(&meta.summary),
            animes::aired_date.eq(aired),
            animes::end_date.eq(parse_date(meta.end_date.as_deref())),
            animes::season_id.eq(season_id),
            animes::updated_at.eq(now),
        ))
        .get_result(conn)?;

    diesel::insert_into(anime_external_ids::table)
        .values(NewAnimeExternalId {
            anime_id,
            namespace: id.namespace.clone(),
            external_id: id.id.clone(),
            source: "fetcher".to_string(),
        })
        .execute(conn)?;

    store_covers(
        conn,
        work.work_id,
        None,
        &meta.series.cover_images,
        fetched.module_id,
    )?;
    store_covers(
        conn,
        work.work_id,
        Some(anime_id),
        &meta.cover_images,
        fetched.module_id,
    )?;
    tracing::info!(
        "Season {} ({} S{}) bound to {}",
        anime_id,
        work.title,
        series_no,
        id
    );
    Ok(SeasonResolution::Resolved(anime))
}

/// 依系列身分找作品；沒有就認領同名、尚未綁定身分的作品；都沒有才建立。
/// 名稱依 spec 第 3 節，`manual` 的名稱永不覆寫。
fn find_or_create_series(conn: &mut PgConnection, series: &SeriesMeta) -> QueryResult<AnimeWork> {
    let by_identity: Option<AnimeWork> = anime_works::table
        .filter(anime_works::series_namespace.eq(&series.namespace))
        .filter(anime_works::series_external_id.eq(&series.external_id))
        .first(conn)
        .optional()?;
    if let Some(work) = by_identity {
        return Ok(work);
    }

    let premiered = parse_date(series.air_date.as_deref());
    let candidates = [
        series.title.clone(),
        premiered
            .map(|d| format!("{} ({})", series.title, d.year()))
            .unwrap_or_else(|| format!("{} ({})", series.title, series.external_id)),
        format!(
            "{} ({}/{})",
            series.title, series.namespace, series.external_id
        ),
    ];
    let mut title = None;
    for candidate in candidates {
        let same_title: Option<AnimeWork> = anime_works::table
            .filter(anime_works::title.eq(&candidate))
            .first(conn)
            .optional()?;
        match same_title {
            Some(work) if work.series_namespace.is_none() => {
                let title_source = if work.title_source == "manual" {
                    "manual"
                } else {
                    "bgm"
                };
                return identify_series(conn, work.work_id, series, title_source, premiered);
            }
            Some(_) => continue,
            None => {
                title = Some(candidate);
                break;
            }
        }
    }
    let title = title.ok_or(DieselError::NotFound)?;
    let now = Utc::now().naive_utc();
    let work_id: i32 = diesel::insert_into(anime_works::table)
        .values(NewAnimeWork {
            title,
            created_at: now,
            updated_at: now,
        })
        .returning(anime_works::work_id)
        .get_result(conn)?;
    identify_series(conn, work_id, series, "bgm", premiered)
}

fn identify_series(
    conn: &mut PgConnection,
    work_id: i32,
    series: &SeriesMeta,
    title_source: &str,
    premiered: Option<NaiveDate>,
) -> QueryResult<AnimeWork> {
    diesel::update(anime_works::table.find(work_id))
        .set((
            anime_works::series_namespace.eq(&series.namespace),
            anime_works::series_external_id.eq(&series.external_id),
            anime_works::title_source.eq(title_source),
            anime_works::description.eq(&series.summary),
            anime_works::premiered.eq(premiered),
            anime_works::is_active.eq(true),
            anime_works::soft_deleted_at.eq(None::<chrono::NaiveDateTime>),
            anime_works::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result(conn)
}

/// 封面：`anime_id = None` 是系列海報，作品還沒有預設封面時第一張設為預設。
fn store_covers(
    conn: &mut PgConnection,
    work_id: i32,
    anime_id: Option<i32>,
    covers: &[CoverImage],
    module_id: Option<i32>,
) -> QueryResult<()> {
    let mut needs_default = anime_id.is_none()
        && !diesel::select(diesel::dsl::exists(
            anime_cover_images::table
                .filter(anime_cover_images::work_id.eq(work_id))
                .filter(anime_cover_images::is_default.eq(true)),
        ))
        .get_result::<bool>(conn)?;
    for cover in covers {
        let inserted = diesel::insert_into(anime_cover_images::table)
            .values(NewAnimeCoverImage {
                work_id,
                anime_id,
                image_url: cover.url.clone(),
                service_module_id: module_id,
                source_name: cover.source.clone(),
                is_default: needs_default,
                created_at: Utc::now().naive_utc(),
            })
            .on_conflict_do_nothing()
            .execute(conn)?;
        needs_default &= inserted == 0;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_db {
    use diesel::prelude::*;

    /// 開發資料庫連線（`DATABASE_TEST_URL`），整個測試在 rollback 的 transaction 裡跑。
    pub fn conn() -> PgConnection {
        let url = std::env::var("DATABASE_TEST_URL")
            .expect("DATABASE_TEST_URL must point to a migrated dev database");
        let mut conn = PgConnection::establish(&url).expect("connect to DATABASE_TEST_URL");
        conn.begin_test_transaction()
            .expect("begin test transaction");
        conn
    }

    pub fn insert_module(conn: &mut PgConnection, name: &str, enabled: bool) -> i32 {
        diesel::sql_query(
            "INSERT INTO service_modules (module_type, name, version, is_enabled, priority, base_url, created_at, updated_at) \
             VALUES ('metadata', $1, '1', $2, 50, 'http://m', now(), now())",
        )
        .bind::<diesel::sql_types::Text, _>(name)
        .bind::<diesel::sql_types::Bool, _>(enabled)
        .execute(conn)
        .unwrap();
        crate::schema::service_modules::table
            .filter(crate::schema::service_modules::name.eq(name))
            .select(crate::schema::service_modules::module_id)
            .first(conn)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::db::AnimeWork;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_metadata_service_registering_twice_declares_its_namespaces_once() {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-a", true);
        save_metadata_namespaces(&mut conn, "meta-a", &["bgm".to_string()]).unwrap();
        save_metadata_namespaces(&mut conn, "meta-a", &["bgm".to_string()]).unwrap();
        assert_eq!(registered_namespaces(&mut conn).unwrap(), set(&["bgm"]));
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_new_declaration_replaces_the_old_one() {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-a", true);
        save_metadata_namespaces(&mut conn, "meta-a", &["bgm".to_string()]).unwrap();
        save_metadata_namespaces(&mut conn, "meta-a", &["tmdb".to_string()]).unwrap();
        assert_eq!(registered_namespaces(&mut conn).unwrap(), set(&["tmdb"]));
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_disabled_service_resolves_nothing() {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-off", false);
        save_metadata_namespaces(&mut conn, "meta-off", &["bgm".to_string()]).unwrap();
        assert!(registered_namespaces(&mut conn).unwrap().is_empty());
    }

    #[test]
    fn only_registered_namespaces_are_usable() {
        let ids = vec![
            ExternalId::new("mikan", "3822"),
            ExternalId::new("bgm", "548818"),
        ];
        assert_eq!(
            usable_ids(&ids, &set(&["bgm"])),
            vec![ExternalId::new("bgm", "548818")]
        );
        assert!(usable_ids(&ids, &set(&[])).is_empty());
    }

    // ---------- 季與系列的解析 ----------

    struct FakeLookup {
        metas: HashMap<String, SeasonMeta>,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl MetadataLookup for FakeLookup {
        async fn enrich_anime(&self, id: &ExternalId) -> Result<Option<FetchedMeta>, String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.metas.get(&id.id).cloned().map(|meta| FetchedMeta {
                meta,
                module_id: None,
            }))
        }
    }

    fn cover(url: &str) -> CoverImage {
        CoverImage {
            url: url.to_string(),
            source: "bangumi".to_string(),
        }
    }

    fn series(id: &str, title: &str) -> SeriesMeta {
        SeriesMeta {
            namespace: "bgm".to_string(),
            external_id: id.to_string(),
            title: title.to_string(),
            original_title: format!("{title} (ja)"),
            summary: Some(format!("{title} 簡介")),
            air_date: Some("2025-01-05".to_string()),
            cover_images: vec![cover(&format!("https://img/{id}.jpg"))],
        }
    }

    fn season(id: &str, title_cn: &str, air_date: &str, series: SeriesMeta) -> SeasonMeta {
        SeasonMeta {
            namespace: "bgm".to_string(),
            external_id: id.to_string(),
            title: format!("{title_cn} (ja)"),
            title_cn: title_cn.to_string(),
            summary: Some(format!("{title_cn} 簡介")),
            air_date: Some(air_date.to_string()),
            end_date: None,
            cover_images: vec![cover(&format!("https://img/{id}.jpg"))],
            series,
        }
    }

    /// 金牌得主兩季（同系列）與欺詐遊戲
    fn lookup() -> FakeLookup {
        let medalist = series("430699", "金牌得主");
        let metas = [
            season("430699", "金牌得主", "2025-01-05", medalist.clone()),
            season("548818", "金牌得主 第二季", "2026-01-24", medalist),
            season(
                "580133",
                "欺诈游戏",
                "2026-04-06",
                series("580133", "欺诈游戏"),
            ),
        ]
        .into_iter()
        .map(|m| (m.external_id.clone(), m))
        .collect();
        FakeLookup {
            metas,
            calls: Default::default(),
        }
    }

    fn bgm(id: &str) -> Vec<ExternalId> {
        vec![ExternalId::new("mikan", "1"), ExternalId::new("bgm", id)]
    }

    async fn resolve(
        conn: &mut PgConnection,
        lookup: &FakeLookup,
        ids: &[ExternalId],
        series_no: i32,
    ) -> SeasonResolution {
        let context = IdentityContext::prepare(conn, lookup, std::slice::from_ref(&ids.to_vec()))
            .await
            .unwrap();
        resolve_season(conn, &context, ids, series_no).unwrap()
    }

    fn resolved(r: SeasonResolution) -> Anime {
        match r {
            SeasonResolution::Resolved(anime) => anime,
            other => panic!("expected a season, got {:?}", other),
        }
    }

    fn work(conn: &mut PgConnection, work_id: i32) -> AnimeWork {
        anime_works::table.find(work_id).first(conn).unwrap()
    }

    fn with_bgm_service(conn: &mut PgConnection) {
        test_db::insert_module(conn, "meta-bgm", true);
        save_metadata_namespaces(conn, "meta-bgm", &["bgm".to_string()]).unwrap();
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn the_same_bgm_id_always_lands_on_one_season() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let lookup = lookup();
        let first = resolved(resolve(&mut conn, &lookup, &bgm("548818"), 2).await);
        // 第二次即使 parser 給了不同季數，身分仍決定季
        let second = resolved(resolve(&mut conn, &lookup, &bgm("548818"), 1).await);
        assert_eq!(first.anime_id, second.anime_id);
        assert_eq!(first.title.as_deref(), Some("金牌得主 第二季"));
        assert_eq!(first.aired_date, NaiveDate::from_ymd_opt(2026, 1, 24));
        assert_eq!(first.description.as_deref(), Some("金牌得主 第二季 簡介"));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn seasons_of_one_series_share_a_work_named_after_the_series() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let lookup = lookup();
        let s2 = resolved(resolve(&mut conn, &lookup, &bgm("548818"), 2).await);
        let s1 = resolved(resolve(&mut conn, &lookup, &bgm("430699"), 1).await);
        assert_eq!(s1.work_id, s2.work_id);
        assert_ne!(s1.anime_id, s2.anime_id);
        let w = work(&mut conn, s1.work_id);
        assert_eq!(w.title, "金牌得主");
        assert_eq!(w.title_source, "bgm");
        assert_eq!(w.series_external_id.as_deref(), Some("430699"));
        assert_eq!(w.description.as_deref(), Some("金牌得主 簡介"));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn the_season_is_filed_under_its_broadcast_quarter() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let s2 = resolved(resolve(&mut conn, &lookup(), &bgm("548818"), 2).await);
        let (year, name): (i32, String) = seasons::table
            .find(s2.season_id)
            .select((seasons::year, seasons::season))
            .first(&mut conn)
            .unwrap();
        assert_eq!((year, name.as_str()), (2026, "winter"));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn ids_no_metadata_service_resolves_are_ignored() {
        let mut conn = test_db::conn();
        // 沒有任何 metadata service 宣告 bgm
        let lookup = lookup();
        let r = resolve(&mut conn, &lookup, &bgm("548818"), 2).await;
        assert!(matches!(r, SeasonResolution::NoIdentity));
        assert_eq!(lookup.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn an_unknown_subject_has_no_identity() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let r = resolve(&mut conn, &lookup(), &bgm("999999"), 1).await;
        assert!(matches!(r, SeasonResolution::NoIdentity));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn a_season_number_taken_by_another_subject_is_a_conflict() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let lookup = lookup();
        resolved(resolve(&mut conn, &lookup, &bgm("430699"), 1).await);
        // parser 把第二季也解析成第 1 季
        let r = resolve(&mut conn, &lookup, &bgm("548818"), 1).await;
        assert!(matches!(r, SeasonResolution::Conflict(_)));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn an_existing_work_with_the_series_title_is_adopted_not_duplicated() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let now = chrono::Utc::now().naive_utc();
        let existing: AnimeWork = diesel::insert_into(anime_works::table)
            .values(NewAnimeWork {
                title: "欺诈游戏".to_string(),
                created_at: now,
                updated_at: now,
            })
            .get_result(&mut conn)
            .unwrap();
        let s = resolved(resolve(&mut conn, &lookup(), &bgm("580133"), 1).await);
        assert_eq!(s.work_id, existing.work_id);
        let w = work(&mut conn, existing.work_id);
        assert_eq!(w.series_external_id.as_deref(), Some("580133"));
        assert_eq!(w.title_source, "bgm");
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn a_manually_named_series_keeps_its_name() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let lookup = lookup();
        let s1 = resolved(resolve(&mut conn, &lookup, &bgm("430699"), 1).await);
        diesel::update(anime_works::table.find(s1.work_id))
            .set((
                anime_works::title.eq("舞冰的祈願"),
                anime_works::title_source.eq("manual"),
            ))
            .execute(&mut conn)
            .unwrap();
        let s2 = resolved(resolve(&mut conn, &lookup, &bgm("548818"), 2).await);
        assert_eq!(s2.work_id, s1.work_id);
        assert_eq!(work(&mut conn, s1.work_id).title, "舞冰的祈願");
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn series_and_season_covers_are_stored() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let s2 = resolved(resolve(&mut conn, &lookup(), &bgm("548818"), 2).await);
        let covers: Vec<(Option<i32>, String, bool)> = anime_cover_images::table
            .filter(anime_cover_images::work_id.eq(s2.work_id))
            .select((
                anime_cover_images::anime_id,
                anime_cover_images::image_url,
                anime_cover_images::is_default,
            ))
            .order(anime_cover_images::cover_id)
            .load(&mut conn)
            .unwrap();
        assert_eq!(
            covers,
            vec![
                (None, "https://img/430699.jpg".to_string(), true),
                (
                    Some(s2.anime_id),
                    "https://img/548818.jpg".to_string(),
                    false
                ),
            ]
        );
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn known_seasons_are_not_looked_up_again() {
        let mut conn = test_db::conn();
        with_bgm_service(&mut conn);
        let lookup = lookup();
        resolved(resolve(&mut conn, &lookup, &bgm("548818"), 2).await);
        resolved(resolve(&mut conn, &lookup, &bgm("548818"), 2).await);
        assert_eq!(lookup.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn broadcast_quarters() {
        let quarter = |m| season_of(NaiveDate::from_ymd_opt(2026, m, 1).unwrap());
        assert_eq!(quarter(1), (2026, "winter"));
        assert_eq!(quarter(4), (2026, "spring"));
        assert_eq!(quarter(7), (2026, "summer"));
        assert_eq!(quarter(12), (2026, "fall"));
    }
}
