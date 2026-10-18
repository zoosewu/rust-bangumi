//! 回填既有資料的作品身分（spec 第 10 節）。
//!
//! 每個訂閱向 fetcher 取得身分、向 metadata 取得內容後，在同一個 transaction 裡：
//! 把訂閱的 link 改掛到身分決定的季、合併同一集的重複下載（保留合集版）、
//! soft delete 沒有任何 link 的作品。dry-run 時整個 transaction 在最後 rollback，
//! 報表就是實際執行的結果。不搬檔案：媒體庫整理留給之後的 resync。

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use serde::Serialize;
use shared::ExternalId;

use crate::db::DbPool;
use crate::schema::{
    anime_links, anime_works, animes, raw_anime_items, service_modules, subscriptions,
};
use crate::services::identity::{
    resolve_season, IdentityContext, MetadataLookup, SeasonResolution,
};

/// 訂閱的作品身分從哪裡來（fetcher 的 `/identity/resolve`）。
#[async_trait]
pub trait IdentitySource: Send + Sync {
    async fn resolve(&self, fetcher_id: i32, source_url: &str) -> Result<Vec<ExternalId>, String>;
}

pub struct HttpIdentitySource {
    db: DbPool,
    http: reqwest::Client,
}

impl HttpIdentitySource {
    pub fn new(db: DbPool) -> Self {
        Self {
            db,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl IdentitySource for HttpIdentitySource {
    async fn resolve(&self, fetcher_id: i32, source_url: &str) -> Result<Vec<ExternalId>, String> {
        let base_url: String = {
            let mut conn = self.db.get().map_err(|e| e.to_string())?;
            service_modules::table
                .find(fetcher_id)
                .select(service_modules::base_url)
                .first(&mut conn)
                .map_err(|e| format!("fetcher {} not found: {}", fetcher_id, e))?
        };
        #[derive(serde::Deserialize)]
        struct Resolved {
            external_ids: Vec<ExternalId>,
        }
        let resp = self
            .http
            .post(format!("{}/identity/resolve", base_url))
            .json(&serde_json::json!({ "source_url": source_url }))
            .send()
            .await
            .map_err(|e| format!("fetcher unreachable: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("fetcher returned HTTP {}", resp.status()));
        }
        resp.json::<Resolved>()
            .await
            .map(|r| r.external_ids)
            .map_err(|e| format!("invalid fetcher response: {}", e))
    }
}

#[derive(Debug, Serialize)]
pub struct BackfillReport {
    pub applied: bool,
    pub subscriptions: Vec<SubscriptionBackfill>,
    pub merged_duplicates: Vec<MergedDuplicate>,
    pub soft_deleted_works: Vec<WorkRef>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionBackfill {
    pub subscription_id: i32,
    pub name: Option<String>,
    pub external_ids: Vec<String>,
    /// `bound` | `no_identity` | `conflict` | `error`
    pub outcome: String,
    pub detail: Option<String>,
    pub season: Option<SeasonRef>,
    /// 這個訂閱的 link 原本所在的作品名稱
    pub previous_works: Vec<String>,
    pub links_moved: usize,
}

#[derive(Debug, Serialize)]
pub struct SeasonRef {
    pub anime_id: i32,
    pub series_no: i32,
    pub series_title: String,
    pub season_title: Option<String>,
    pub aired_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Serialize, QueryableByName)]
pub struct MergedDuplicate {
    #[diesel(sql_type = Integer)]
    pub anime_id: i32,
    #[diesel(sql_type = Integer)]
    pub episode_no: i32,
    #[diesel(sql_type = Integer)]
    pub kept_download_id: i32,
    /// 標為 resolved 的另一份 link
    #[diesel(sql_type = Integer)]
    pub resolved_link_id: i32,
    /// 它的下載（標為 cancelled）；沒下載過時為 None
    #[diesel(sql_type = diesel::sql_types::Nullable<Integer>)]
    pub cancelled_download_id: Option<i32>,
}

#[derive(Debug, Serialize, QueryableByName)]
pub struct WorkRef {
    #[diesel(sql_type = Integer)]
    pub work_id: i32,
    #[diesel(sql_type = Text)]
    pub title: String,
}

struct SubscriptionRow {
    subscription_id: i32,
    fetcher_id: i32,
    source_url: String,
    name: Option<String>,
}

/// transaction 內的錯誤；`DryRun` 用來在最後 rollback 並帶出報表。
enum TxError {
    Db(diesel::result::Error),
    Other(String),
    DryRun(BackfillReport),
}

impl From<diesel::result::Error> for TxError {
    fn from(e: diesel::result::Error) -> Self {
        TxError::Db(e)
    }
}

impl From<String> for TxError {
    fn from(e: String) -> Self {
        TxError::Other(e)
    }
}

/// 訂閱 link 所在的季中，link 最多的那一季的季數（parser 解析的季數）
fn dominant_series_no(conn: &mut PgConnection, subscription_id: i32) -> QueryResult<Option<i32>> {
    anime_links::table
        .inner_join(raw_anime_items::table)
        .inner_join(animes::table)
        .filter(raw_anime_items::subscription_id.eq(subscription_id))
        .group_by(animes::series_no)
        .select(animes::series_no)
        .order((diesel::dsl::count_star().desc(), animes::series_no.asc()))
        .first(conn)
        .optional()
}

fn previous_works(conn: &mut PgConnection, subscription_id: i32) -> QueryResult<Vec<String>> {
    anime_links::table
        .inner_join(raw_anime_items::table)
        .inner_join(animes::table.inner_join(anime_works::table))
        .filter(raw_anime_items::subscription_id.eq(subscription_id))
        .select(anime_works::title)
        .distinct()
        .order(anime_works::title)
        .load(conn)
}

fn season_ref(conn: &mut PgConnection, anime_id: i32) -> QueryResult<SeasonRef> {
    let (series_no, season_title, aired_date, series_title) = animes::table
        .inner_join(anime_works::table)
        .filter(animes::anime_id.eq(anime_id))
        .select((
            animes::series_no,
            animes::title,
            animes::aired_date,
            anime_works::title,
        ))
        .first::<(i32, Option<String>, Option<chrono::NaiveDate>, String)>(conn)?;
    Ok(SeasonRef {
        anime_id,
        series_no,
        series_title,
        season_title,
        aired_date,
    })
}

/// 被改掛到別一季的 link 與它原本所在的季
#[derive(QueryableByName)]
struct MovedLink {
    #[diesel(sql_type = Integer)]
    link_id: i32,
    #[diesel(sql_type = Integer)]
    origin: i32,
}

fn bind_subscription(
    conn: &mut PgConnection,
    context: &IdentityContext,
    sub: &SubscriptionRow,
    ids: &[ExternalId],
    moved: &mut Vec<MovedLink>,
) -> Result<SubscriptionBackfill, TxError> {
    let mut report = SubscriptionBackfill {
        subscription_id: sub.subscription_id,
        name: sub.name.clone(),
        external_ids: ids.iter().map(ExternalId::to_string).collect(),
        outcome: "no_identity".to_string(),
        detail: None,
        season: None,
        previous_works: previous_works(conn, sub.subscription_id)?,
        links_moved: 0,
    };
    let series_no = dominant_series_no(conn, sub.subscription_id)?.unwrap_or(1);
    match resolve_season(conn, context, ids, series_no)? {
        SeasonResolution::Resolved(season) => {
            let links: Vec<MovedLink> = diesel::sql_query(
                "UPDATE anime_links l SET anime_id = $1 \
                 FROM (SELECT link_id, anime_id FROM anime_links \
                       WHERE anime_id <> $1 AND raw_item_id IN \
                             (SELECT item_id FROM raw_anime_items WHERE subscription_id = $2)) old \
                 WHERE l.link_id = old.link_id \
                 RETURNING l.link_id, old.anime_id AS origin",
            )
            .bind::<Integer, _>(season.anime_id)
            .bind::<Integer, _>(sub.subscription_id)
            .load(conn)?;
            report.links_moved = links.len();
            moved.extend(links);
            diesel::update(
                raw_anime_items::table
                    .filter(raw_anime_items::subscription_id.eq(sub.subscription_id)),
            )
            .set(
                raw_anime_items::external_ids.eq(ids
                    .iter()
                    .map(|id| Some(id.to_string()))
                    .collect::<Vec<_>>()),
            )
            .execute(conn)?;
            report.outcome = "bound".to_string();
            report.season = Some(season_ref(conn, season.anime_id)?);
        }
        SeasonResolution::Conflict(reason) => {
            report.outcome = "conflict".to_string();
            report.detail = Some(reason);
        }
        SeasonResolution::NoIdentity => {
            report.detail = Some("no metadata for these ids".to_string());
        }
    }
    Ok(report)
}

/// 這次回填把兩季併成一季而產生的重複：同一季、同一字幕組、同一集有兩個 link，
/// 來自不同的原本季，且其中一份已經下載 → 保留已下載的那份（合集版優先，其次較早的下載），
/// 另一份 link 標為 resolved、有下載的話標為 cancelled。
/// 原本就在同一季的重複（使用者待決的衝突）不動。
fn merge_duplicate_downloads(
    conn: &mut PgConnection,
    seasons: &[i32],
    moved: &[MovedLink],
) -> QueryResult<Vec<MergedDuplicate>> {
    let moved_ids: Vec<i32> = moved.iter().map(|m| m.link_id).collect();
    let origins: Vec<i32> = moved.iter().map(|m| m.origin).collect();
    diesel::sql_query(
        "WITH moved AS ( \
             SELECT * FROM unnest($2::int[], $3::int[]) AS m(link_id, origin) \
         ), live AS ( \
             SELECT l.link_id, l.anime_id, l.group_id, l.episode_no, d.download_id, \
                    COALESCE(m.origin, l.anime_id) AS origin, \
                    (l.source_hash LIKE '%#ep%') AS is_batch \
             FROM anime_links l \
             LEFT JOIN LATERAL ( \
                 SELECT download_id FROM downloads \
                 WHERE link_id = l.link_id AND status NOT IN ('failed', 'cancelled') \
                 ORDER BY download_id DESC LIMIT 1) d ON true \
             LEFT JOIN moved m ON m.link_id = l.link_id \
             WHERE l.anime_id = ANY($1) AND l.link_status = 'active' AND l.filtered_flag = false \
         ), ranked AS ( \
             SELECT *, ROW_NUMBER() OVER w AS rank, \
                    FIRST_VALUE(download_id) OVER w AS kept, \
                    FIRST_VALUE(origin) OVER w AS kept_origin \
             FROM live \
             WINDOW w AS (PARTITION BY anime_id, group_id, episode_no \
                          ORDER BY (download_id IS NOT NULL) DESC, is_batch DESC, download_id) \
         ), losers AS ( \
             SELECT * FROM ranked \
             WHERE rank > 1 AND origin <> kept_origin AND kept IS NOT NULL \
         ), cancelled AS ( \
             UPDATE downloads d SET status = 'cancelled', \
                    error_message = 'merged duplicate of download ' || losers.kept, \
                    updated_at = NOW() \
             FROM losers WHERE d.download_id = losers.download_id \
         ), resolved AS ( \
             UPDATE anime_links SET link_status = 'resolved' \
             WHERE link_id IN (SELECT link_id FROM losers) \
         ) \
         SELECT anime_id, episode_no, kept AS kept_download_id, link_id AS resolved_link_id, \
                download_id AS cancelled_download_id \
         FROM losers ORDER BY anime_id, episode_no, link_id",
    )
    .bind::<diesel::sql_types::Array<Integer>, _>(seasons)
    .bind::<diesel::sql_types::Array<Integer>, _>(moved_ids)
    .bind::<diesel::sql_types::Array<Integer>, _>(origins)
    .load(conn)
}

/// 沒有任何 link 的作品 soft delete（可逆；之後同名項目出現會重新啟用）。
fn soft_delete_unreachable_works(conn: &mut PgConnection) -> QueryResult<Vec<WorkRef>> {
    diesel::sql_query(
        "UPDATE anime_works w SET is_active = false, soft_deleted_at = NOW(), updated_at = NOW() \
         WHERE w.is_active AND NOT EXISTS ( \
             SELECT 1 FROM animes a JOIN anime_links l ON l.anime_id = a.anime_id \
             WHERE a.work_id = w.work_id) \
         RETURNING w.work_id, w.title",
    )
    .load(conn)
    .map(|mut works: Vec<WorkRef>| {
        works.sort_by_key(|w| w.work_id);
        works
    })
}

pub async fn run_backfill(
    db: &DbPool,
    source: &dyn IdentitySource,
    lookup: &dyn MetadataLookup,
    apply: bool,
) -> Result<BackfillReport, String> {
    let mut conn = db.get().map_err(|e| e.to_string())?;
    let subs: Vec<SubscriptionRow> = subscriptions::table
        .filter(diesel::dsl::exists(
            anime_links::table
                .inner_join(raw_anime_items::table)
                .filter(raw_anime_items::subscription_id.eq(subscriptions::subscription_id)),
        ))
        .order(subscriptions::subscription_id)
        .select((
            subscriptions::subscription_id,
            subscriptions::fetcher_id,
            subscriptions::source_url,
            subscriptions::name,
        ))
        .load::<(i32, i32, String, Option<String>)>(&mut conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(
            |(subscription_id, fetcher_id, source_url, name)| SubscriptionRow {
                subscription_id,
                fetcher_id,
                source_url,
                name,
            },
        )
        .collect();

    // 網路查詢都在 transaction 之前做完
    let mut resolved: Vec<(SubscriptionRow, Result<Vec<ExternalId>, String>)> = Vec::new();
    for sub in subs {
        let ids = source.resolve(sub.fetcher_id, &sub.source_url).await;
        resolved.push((sub, ids));
    }
    let all_ids: Vec<Vec<ExternalId>> = resolved
        .iter()
        .filter_map(|(_, ids)| ids.as_ref().ok().cloned())
        .collect();
    let context = IdentityContext::prepare(&mut conn, lookup, &all_ids).await?;

    let outcome = conn.transaction::<BackfillReport, TxError, _>(|conn| {
        let mut subscriptions = Vec::new();
        let mut moved = Vec::new();
        for (sub, ids) in &resolved {
            let report = match ids {
                Ok(ids) => bind_subscription(conn, &context, sub, ids, &mut moved)?,
                Err(e) => SubscriptionBackfill {
                    subscription_id: sub.subscription_id,
                    name: sub.name.clone(),
                    external_ids: vec![],
                    outcome: "error".to_string(),
                    detail: Some(e.clone()),
                    season: None,
                    previous_works: previous_works(conn, sub.subscription_id)?,
                    links_moved: 0,
                },
            };
            subscriptions.push(report);
        }
        let seasons: Vec<i32> = subscriptions
            .iter()
            .filter_map(|s| s.season.as_ref().map(|season| season.anime_id))
            .collect();
        let report = BackfillReport {
            applied: apply,
            merged_duplicates: merge_duplicate_downloads(conn, &seasons, &moved)?,
            soft_deleted_works: soft_delete_unreachable_works(conn)?,
            subscriptions,
        };
        if apply {
            Ok(report)
        } else {
            Err(TxError::DryRun(report))
        }
    });
    match outcome {
        Ok(report) | Err(TxError::DryRun(report)) => Ok(report),
        Err(TxError::Db(e)) => Err(format!("Backfill failed and was rolled back: {}", e)),
        Err(TxError::Other(e)) => Err(format!("Backfill failed and was rolled back: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::identity::{save_metadata_namespaces, test_db};
    use std::collections::HashMap;

    struct FakeSource(HashMap<String, Vec<ExternalId>>);

    #[async_trait]
    impl IdentitySource for FakeSource {
        async fn resolve(&self, _fetcher_id: i32, url: &str) -> Result<Vec<ExternalId>, String> {
            self.0
                .get(url)
                .cloned()
                .ok_or_else(|| "fetcher unreachable".to_string())
        }
    }

    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type = Integer)]
        id: i32,
    }

    fn insert(conn: &mut PgConnection, sql: &str) -> i32 {
        diesel::sql_query(sql).get_result::<Id>(conn).unwrap().id
    }

    fn work(conn: &mut PgConnection, title: &str, series_no: i32) -> (i32, i32) {
        let work = insert(conn, &format!("INSERT INTO anime_works (title, created_at, updated_at) VALUES ('{title}', now(), now()) RETURNING work_id AS id"));
        let season = insert(conn, "INSERT INTO seasons (year, season, created_at) VALUES (2025, 'unknown', now()) ON CONFLICT (year, season) DO UPDATE SET year = EXCLUDED.year RETURNING season_id AS id");
        let anime = insert(conn, &format!("INSERT INTO animes (work_id, series_no, season_id, created_at, updated_at) VALUES ({work}, {series_no}, {season}, now(), now()) RETURNING anime_id AS id"));
        (work, anime)
    }

    /// 訂閱的一個 raw item 與它展開的 link，每個 link 一筆 synced 下載；回傳 download id
    fn release(
        conn: &mut PgConnection,
        sub: i32,
        anime: i32,
        group: i32,
        name: &str,
        eps: std::ops::RangeInclusive<i32>,
    ) -> Vec<i32> {
        let batch = eps.start() != eps.end();
        let item = insert(conn, &format!("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, created_at) VALUES ('{name}', '{name}', {sub}, 'parsed', now()) RETURNING item_id AS id"));
        eps.map(|ep| {
            let hash = if batch { format!("{name}#ep{ep}") } else { name.to_string() };
            let link = insert(conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, url, source_hash, raw_item_id) VALUES ({anime}, {group}, {ep}, '{name}', '{hash}', {item}) RETURNING link_id AS id"));
            insert(conn, &format!("INSERT INTO downloads (link_id, downloader_type, status) VALUES ({link}, 'qbittorrent', 'synced') RETURNING download_id AS id"))
        })
        .collect()
    }

    /// 沒下載過的單集 link；回傳 link id
    fn undownloaded(
        conn: &mut PgConnection,
        sub: i32,
        anime: i32,
        group: i32,
        name: &str,
        ep: i32,
    ) -> i32 {
        let item = insert(conn, &format!("INSERT INTO raw_anime_items (title, download_url, subscription_id, status, created_at) VALUES ('{name}', '{name}', {sub}, 'parsed', now()) RETURNING item_id AS id"));
        insert(conn, &format!("INSERT INTO anime_links (anime_id, group_id, episode_no, url, source_hash, raw_item_id) VALUES ({anime}, {group}, {ep}, '{name}', '{name}', {item}) RETURNING link_id AS id"))
    }

    struct Fixture {
        sub: i32,
        failing_sub: i32,
        single_downloads: Vec<i32>,
        batch_downloads: Vec<i32>,
        variant_download: i32,
        undownloaded_link: i32,
        parser_work: i32,
        batch_work: i32,
        orphan_work: i32,
        source: FakeSource,
    }

    /// 產品環境的形狀：同一訂閱的單集與合集被拆成兩個作品，各自下載過同兩集；另有一個殘骸作品
    fn fixture(conn: &mut PgConnection) -> Fixture {
        test_db::insert_module(conn, "meta-bgm", true);
        save_metadata_namespaces(conn, "meta-bgm", &["bgm".to_string()]).unwrap();
        let fetcher = insert(conn, "INSERT INTO service_modules (module_type, name, version, is_enabled, priority, base_url, created_at, updated_at) VALUES ('fetcher', 'backfill-fetcher', '1', true, 1, 'http://f', now(), now()) RETURNING module_id AS id");
        let sub = insert(conn, &format!("INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) VALUES ({fetcher}, 'https://mikan/3822', '金牌得主', now(), now()) RETURNING subscription_id AS id"));
        let failing_sub = insert(conn, &format!("INSERT INTO subscriptions (fetcher_id, source_url, name, created_at, updated_at) VALUES ({fetcher}, 'https://mikan/unreachable', '連不上', now(), now()) RETURNING subscription_id AS id"));
        let group = insert(conn, "INSERT INTO subtitle_groups (group_name) VALUES ('回填測試組') RETURNING group_id AS id");

        let (parser_work, single_anime) = work(conn, "金牌得主的 AI 名稱", 2);
        let (batch_work, batch_anime) = work(conn, "Medalist S2 合集", 2);
        let (orphan_work, _) = work(conn, "殘骸", 1);
        let (_, failing_anime) = work(conn, "連不上的作品", 1);

        let single_downloads = (1..=2)
            .flat_map(|ep| {
                release(
                    conn,
                    sub,
                    single_anime,
                    group,
                    &format!("single-{ep}"),
                    ep..=ep,
                )
            })
            .collect();
        // 第 3 集只有合集版下載過，單集版從沒下載
        let undownloaded_link = undownloaded(conn, sub, single_anime, group, "single-3", 3);
        let batch_downloads = release(conn, sub, batch_anime, group, "batch", 1..=3);
        // 原本就在同一季、使用者還沒決定的同集兩版本：回填不能替使用者決定
        let variant_download = release(conn, sub, batch_anime, group, "variant-1", 1..=1)[0];
        release(conn, failing_sub, failing_anime, group, "other", 1..=1);

        let source = FakeSource(HashMap::from([(
            "https://mikan/3822".to_string(),
            vec![
                ExternalId::new("mikan", "3822"),
                ExternalId::new("bgm", "548818"),
            ],
        )]));
        Fixture {
            sub,
            failing_sub,
            single_downloads,
            batch_downloads,
            variant_download,
            undownloaded_link,
            parser_work,
            batch_work,
            orphan_work,
            source,
        }
    }

    fn download_status(conn: &mut PgConnection, id: i32) -> String {
        crate::schema::downloads::table
            .find(id)
            .select(crate::schema::downloads::status)
            .first(conn)
            .unwrap()
    }

    async fn run(apply: bool) -> (BackfillReport, Fixture, DbPool) {
        let pool = test_db::pool();
        let fx = fixture(&mut pool.get().unwrap());
        let report = run_backfill(&pool, &fx.source, &test_db::lookup(), apply)
            .await
            .unwrap();
        (report, fx, pool)
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn a_dry_run_reports_everything_and_changes_nothing() {
        let (report, fx, pool) = run(false).await;
        let mut conn = pool.get().unwrap();
        assert!(!report.applied);

        let bound = report
            .subscriptions
            .iter()
            .find(|s| s.subscription_id == fx.sub)
            .unwrap();
        assert_eq!(bound.outcome, "bound");
        assert_eq!(bound.links_moved, 7);
        assert_eq!(
            bound.previous_works,
            vec!["Medalist S2 合集", "金牌得主的 AI 名稱"]
        );
        let season = bound.season.as_ref().unwrap();
        assert_eq!(season.series_title, "金牌得主");
        assert_eq!(season.season_title.as_deref(), Some("金牌得主 第二季"));
        assert_eq!(season.series_no, 2);

        let failing = report
            .subscriptions
            .iter()
            .find(|s| s.subscription_id == fx.failing_sub)
            .unwrap();
        assert_eq!(failing.outcome, "error");

        // 同一集保留合集版，取消單集版；單集版沒下載過的只標為 resolved
        let cancelled: Vec<i32> = report
            .merged_duplicates
            .iter()
            .filter_map(|m| m.cancelled_download_id)
            .collect();
        let kept: Vec<i32> = report
            .merged_duplicates
            .iter()
            .map(|m| m.kept_download_id)
            .collect();
        assert_eq!(cancelled, fx.single_downloads);
        assert_eq!(kept, fx.batch_downloads);
        let third = report
            .merged_duplicates
            .iter()
            .find(|m| m.episode_no == 3)
            .unwrap();
        assert_eq!(third.resolved_link_id, fx.undownloaded_link);
        assert_eq!(third.cancelled_download_id, None);

        let deleted: Vec<i32> = report
            .soft_deleted_works
            .iter()
            .map(|w| w.work_id)
            .collect();
        assert!(deleted.contains(&fx.parser_work));
        assert!(deleted.contains(&fx.batch_work));
        assert!(deleted.contains(&fx.orphan_work));

        // 什麼都沒寫進去
        assert_eq!(download_status(&mut conn, fx.single_downloads[0]), "synced");
        let active: bool = anime_works::table
            .find(fx.orphan_work)
            .select(anime_works::is_active)
            .first(&mut conn)
            .unwrap();
        assert!(active);
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn applying_moves_links_merges_duplicates_and_retires_old_works() {
        let (report, fx, pool) = run(true).await;
        let mut conn = pool.get().unwrap();
        assert!(report.applied);
        let season = report
            .subscriptions
            .iter()
            .find(|s| s.subscription_id == fx.sub)
            .and_then(|s| s.season.as_ref())
            .unwrap()
            .anime_id;

        let on_season: i64 = anime_links::table
            .filter(anime_links::anime_id.eq(season))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(on_season, 7);
        assert_eq!(
            download_status(&mut conn, fx.single_downloads[0]),
            "cancelled"
        );
        assert_eq!(download_status(&mut conn, fx.batch_downloads[0]), "synced");
        // 原本就在同一季的同集另一版本不被合併
        assert_eq!(download_status(&mut conn, fx.variant_download), "synced");

        let ids: Vec<Option<String>> = raw_anime_items::table
            .filter(raw_anime_items::subscription_id.eq(fx.sub))
            .select(raw_anime_items::external_ids)
            .first(&mut conn)
            .unwrap();
        assert_eq!(
            ids,
            vec![
                Some("mikan/3822".to_string()),
                Some("bgm/548818".to_string())
            ]
        );

        let retired: (bool, bool) = anime_works::table
            .find(fx.orphan_work)
            .select((
                anime_works::is_active,
                anime_works::soft_deleted_at.is_not_null(),
            ))
            .first(&mut conn)
            .unwrap();
        assert_eq!(retired, (false, true));
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_TEST_URL"]
    async fn running_it_again_changes_nothing_more() {
        let pool = test_db::pool();
        let fx = fixture(&mut pool.get().unwrap());
        run_backfill(&pool, &fx.source, &test_db::lookup(), true)
            .await
            .unwrap();
        let again = run_backfill(&pool, &fx.source, &test_db::lookup(), true)
            .await
            .unwrap();
        let bound = again
            .subscriptions
            .iter()
            .find(|s| s.subscription_id == fx.sub)
            .unwrap();
        assert_eq!(bound.links_moved, 0);
        assert!(again.merged_duplicates.is_empty());
        assert!(again.soft_deleted_works.is_empty());
    }
}
