# 作品身分確定性化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 讓作品身分來自 fetcher 提供的 namespaced external id（如 `bgm/548818`），而非 AI 從 RSS 標題發明的字串，並回填產品環境的既有錯誤資料。

**Architecture:** fetcher 回報它所知道的全部身分（`["mikan/3822", "bgm/548818"]`）；core 依 metadata service 註冊的 namespace 過濾出可用的 id，以 `UNIQUE (namespace, external_id)` 查表決定要掛到哪一季（`animes`），查無則呼叫 metadata service 取權威資料後建立。title / 簡介 / 封面 / 日期一律由 metadata service 提供，parser 只負責集數、字幕組、畫質。

**Tech Stack:** Rust / Axum / Diesel 2.1 + PostgreSQL 15 / reqwest / scraper 0.20 / regex 1.10

**Spec:** `docs/superpowers/specs/2026-07-15-metadata-identity-design.md`

## Global Constraints

- 所有新增 SQL 遷移置於 `core-service/migrations/YYYY-MM-DD-HHMMSS-<name>/{up,down}.sql`，`down.sql` 必須可逆。
- external id 的線上格式一律為字串 `"{namespace}/{id}"`，不使用巢狀物件。
- Diesel schema (`core-service/src/schema.rs`) 由 `diesel migration run` 自動生成，**不得手改**。
- 提交訊息說明「為什麼」而非「做了什麼」，結尾附 `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`。
- 每個 task 結束前須通過 `cargo fmt` 與 `cargo clippy`。
- 產品環境唯讀：任何回填只能經 CLI 由維運者手動執行，預設 dry-run。
- 既有 `#[serde(default)]` 慣例：所有新增的 payload 欄位必須向後相容舊服務。

---

## File Structure

| 檔案 | 職責 |
|------|------|
| `shared/src/external_id.rs` (新) | `ExternalId` 型別：`"ns/id"` 的解析、格式化、serde |
| `shared/src/models.rs` (改) | `RawAnimeItem.external_ids`、`Capabilities.namespaces` |
| `fetchers/mikanani/src/detail_scraper.rs` (改) | 從 detail HTML 抽 bgm subject id |
| `fetchers/mikanani/src/identity_resolver.rs` (新) | mikan id → external ids，帶快取 |
| `fetchers/mikanani/src/fetch_task.rs` (改) | 把 external ids 掛到 raw items |
| `core-service/migrations/.../up.sql` (新) | 新表與欄位 |
| `core-service/src/services/identity.rs` (新) | 依 external id 解析/建立 anime |
| `core-service/src/handlers/fetcher_results.rs` (改) | 接上 identity 解析 |
| `core-service/src/handlers/services.rs` (改) | metadata namespace 註冊落地 |
| `core-service/src/handlers/pending_identities.rs` (新) | 待認領佇列 API |
| `metadata/src/bangumi_client.rs` (改) | 依 subject id 查詢；刪除 `list[0]` 自動選取 |
| `metadata/src/handlers.rs` (改) | enrich 改吃 external id；新增 candidates |
| `cli/src/commands/backfill.rs` (新) | `backfill-identity --dry-run/--apply` |

---

### Task 1: shared — `ExternalId` 型別與 payload 欄位

**Files:**
- Create: `shared/src/external_id.rs`
- Modify: `shared/src/lib.rs`, `shared/src/models.rs:320-325`, `shared/src/models.rs:59-72`

**Interfaces:**
- Produces: `shared::ExternalId { namespace: String, id: String }`，含 `ExternalId::new(ns, id)`、
  `FromStr`（`"bgm/548818"` → `ExternalId`）、`Display`（反向）、serde 以字串序列化。
  `shared::RawAnimeItem.external_ids: Vec<ExternalId>`。
  `shared::Capabilities.namespaces: Vec<String>`。

- [ ] **Step 1: 寫失敗測試**

建立 `shared/src/external_id.rs`，先只放測試：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_namespace_and_id() {
        let e = ExternalId::from_str("bgm/548818").unwrap();
        assert_eq!(e.namespace, "bgm");
        assert_eq!(e.id, "548818");
    }

    #[test]
    fn round_trips_through_display() {
        let e = ExternalId::from_str("mikan/3822").unwrap();
        assert_eq!(e.to_string(), "mikan/3822");
    }

    #[test]
    fn keeps_slashes_inside_the_id() {
        // 只在第一個 '/' 切開，讓 tmdb/tv/1234 這種階層 id 仍可表達
        let e = ExternalId::from_str("tmdb/tv/1234").unwrap();
        assert_eq!(e.namespace, "tmdb");
        assert_eq!(e.id, "tv/1234");
    }

    #[test]
    fn rejects_input_without_separator() {
        assert!(ExternalId::from_str("bgm").is_err());
    }

    #[test]
    fn rejects_empty_namespace_or_id() {
        assert!(ExternalId::from_str("/548818").is_err());
        assert!(ExternalId::from_str("bgm/").is_err());
    }

    #[test]
    fn serialises_as_a_plain_string() {
        let e = ExternalId::new("bgm", "548818");
        assert_eq!(serde_json::to_string(&e).unwrap(), r#""bgm/548818""#);
        let back: ExternalId = serde_json::from_str(r#""bgm/548818""#).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn rejects_malformed_string_during_deserialisation() {
        assert!(serde_json::from_str::<ExternalId>(r#""bgm""#).is_err());
    }
}
```

- [ ] **Step 2: 執行測試確認失敗**

先在 `shared/src/lib.rs` 的 `pub mod models;` 上方加入：

```rust
pub mod external_id;
pub use external_id::ExternalId;
```

Run: `cargo test -p shared external_id`
Expected: FAIL，`cannot find type ExternalId in this scope`

- [ ] **Step 3: 實作 `ExternalId`**

在 `shared/src/external_id.rs` 的測試模組上方加入：

```rust
use serde::{Deserialize, Serialize};

/// 一季作品在某外部站台的身分，例如 `bgm/548818`。
///
/// namespace 決定哪個 metadata service 能解析它；id 對其他人而言是不透明的。
/// 線上格式固定為 `"{namespace}/{id}"` 字串。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExternalId {
    pub namespace: String,
    pub id: String,
}

impl ExternalId {
    pub fn new(namespace: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            id: id.into(),
        }
    }
}

impl std::fmt::Display for ExternalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.id)
    }
}

impl std::str::FromStr for ExternalId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (namespace, id) = s
            .split_once('/')
            .ok_or_else(|| format!("external id missing '/' separator: {s:?}"))?;
        if namespace.is_empty() || id.is_empty() {
            return Err(format!("external id has empty namespace or id: {s:?}"));
        }
        Ok(Self::new(namespace, id))
    }
}

impl TryFrom<String> for ExternalId {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<ExternalId> for String {
    fn from(e: ExternalId) -> String {
        e.to_string()
    }
}
```

- [ ] **Step 4: 執行測試確認通過**

Run: `cargo test -p shared external_id`
Expected: PASS（7 passed）

- [ ] **Step 5: 加入 payload 欄位**

`shared/src/models.rs:320-325` 改為：

```rust
pub struct RawAnimeItem {
    pub title: String,                   // RSS <title>
    pub description: Option<String>,     // RSS <description>
    pub download_url: String,            // RSS <enclosure> url
    pub pub_date: Option<DateTime<Utc>>, // RSS <pubDate>
    /// fetcher 所知道的全部身分，例如 ["mikan/3822", "bgm/548818"]。
    /// fetcher 不預設誰會使用哪一個；由 core 依已註冊的 namespace 過濾。
    #[serde(default)]
    pub external_ids: Vec<crate::ExternalId>,
}
```

`shared/src/models.rs:70-71`（`Capabilities` 內 `supported_download_types` 之後）加入：

```rust
    /// metadata service 認領的 namespace，例如 ["bgm"]。
    #[serde(default)]
    pub namespaces: Vec<String>,
```

- [ ] **Step 6: 修正所有建構點**

Run: `cargo build --workspace 2>&1 | rg "missing field"`
Expected: 列出所有缺欄位的建構點。逐一補上 `external_ids: vec![]` 或 `namespaces: vec![]`。
已知需要修改：`metadata/src/main.rs:63-70`（`Capabilities` 建構）、
`fetchers/mikanani/src/rss_parser.rs:71`（`RawAnimeItem` 建構）。

- [ ] **Step 7: 全工作區測試**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: 全數 PASS（`#[serde(default)]` 保證舊 payload 仍可反序列化）

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace -- -D warnings
git add shared/ metadata/src/main.rs fetchers/mikanani/src/rss_parser.rs
git commit -m "feat: add namespaced ExternalId as the carrier of work identity

Work identity is currently a string a low-intelligence AI invents from a
single RSS title, which is why prod has a season labelled てんびん. Introduce
a namespaced id so identity can be carried from the fetcher instead of
guessed, and let metadata services declare which namespace they resolve.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: mikanani — 從 detail 頁抽 bgm subject id

**Files:**
- Modify: `fetchers/mikanani/src/detail_scraper.rs`
- Test: 同檔 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 無（純函式）
- Produces: `pub fn parse_bgm_subject_id(html: &str) -> Option<String>`

**背景（已於 2026-07-15 對真實站台實測）：** `https://mikanani.me/Home/Bangumi/3822` 內含

```html
<p class="bangumi-info">Bangumi番组计划链接：<br />
  <a class="w-other-c" target="_blank" href="https://bgm.tv/subject/548818">https://bgm.tv/subject/548818</a></p>
```

注意 mikan 的 `bangumiId=3822` 與 bgm 的 `subject/548818` 是**不同命名空間**，不可混用。

- [ ] **Step 1: 寫失敗測試**

在 `fetchers/mikanani/src/detail_scraper.rs` 的 `mod tests` 內加入：

```rust
    /// Source: https://mikanani.me/Home/Bangumi/3822，擷取自 2026-07-15。
    /// Refresh when mikanani changes its bangumi detail HTML structure.
    static REAL_BGM_LINK_HTML: &str = r#"
        <p class="bangumi-info">放送开始：1/24/2026</p>
        <p class="bangumi-info">官方网站：<br /><a class="w-other-c" target="_blank" href="https://medalist-pr.com/">https://medalist-pr.com/</a></p>
        <p class="bangumi-info">Bangumi番组计划链接：<br /><a class="w-other-c" target="_blank" href="https://bgm.tv/subject/548818">https://bgm.tv/subject/548818</a></p>
    "#;

    #[test]
    fn extracts_bgm_subject_id_from_real_detail_page() {
        assert_eq!(
            parse_bgm_subject_id(REAL_BGM_LINK_HTML),
            Some("548818".to_string())
        );
    }

    #[test]
    fn returns_none_when_the_page_has_no_bgm_link() {
        let html = r#"<p class="bangumi-info">官方网站：<a href="https://example.com/">x</a></p>"#;
        assert_eq!(parse_bgm_subject_id(html), None);
    }

    #[test]
    fn does_not_mistake_the_mikan_id_for_a_bgm_id() {
        // mikan 自己的 id 出現在 URL 裡，但它不是 bgm subject id。
        let html = r#"<a href="/Home/Bangumi/3822">x</a>"#;
        assert_eq!(parse_bgm_subject_id(html), None);
    }

    #[test]
    fn accepts_a_protocol_relative_or_http_link() {
        let html = r#"<a href="http://bgm.tv/subject/325808">x</a>"#;
        assert_eq!(parse_bgm_subject_id(html), Some("325808".to_string()));
    }
```

- [ ] **Step 2: 執行測試確認失敗**

Run: `cargo test -p mikanani-fetcher parse_bgm_subject_id 2>&1 | tail -5`
Expected: FAIL，`cannot find function parse_bgm_subject_id`

（若 package 名不符，先以 `rg '^name' fetchers/mikanani/Cargo.toml` 確認。）

- [ ] **Step 3: 實作**

在 `fetchers/mikanani/src/detail_scraper.rs` 頂部 `use` 之後加入：

```rust
use regex::Regex;
use std::sync::OnceLock;

/// 從 mikan 的 bangumi detail 頁抽出 bgm.tv subject id。
///
/// mikan 在 detail 頁公開一份人工校對過的 bgm 對應（「Bangumi番组计划链接」），
/// 這是本專案取得作品身分的權威來源——遠優於拿 RSS 標題去模糊搜尋。
///
/// 注意：mikan 自身的 `/Home/Bangumi/{id}` 是另一個命名空間，不是 bgm subject id。
pub fn parse_bgm_subject_id(html: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"bgm\.tv/subject/(\d+)").unwrap());
    re.captures(html).map(|c| c[1].to_string())
}
```

在 `fetchers/mikanani/Cargo.toml` 的 `[dependencies]` 加入（regex 已是 workspace dep）：

```toml
regex = { workspace = true }
```

- [ ] **Step 4: 執行測試確認通過**

Run: `cargo test -p mikanani-fetcher parse_bgm_subject_id 2>&1 | tail -5`
Expected: PASS（4 passed）

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p mikanani-fetcher -- -D warnings
git add fetchers/mikanani/
git commit -m "feat: extract the bgm.tv subject link mikan already publishes

mikan maintains a hand-curated mikan->bgm mapping on every bangumi detail
page. Reading it costs one cached HTTP call and resolved 12/12 of prod's
subscriptions correctly, where title-based fuzzy search mislabelled one
season entirely.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: mikanani — mikan id → external ids（帶快取）

**Files:**
- Create: `fetchers/mikanani/src/identity_resolver.rs`
- Modify: `fetchers/mikanani/src/lib.rs`（加 `pub mod identity_resolver;`）

**Interfaces:**
- Consumes: `parse_bgm_subject_id`（Task 2）、`shared::ExternalId`（Task 1）
- Produces:
  - `pub fn mikan_id_from_rss_url(rss_url: &str) -> Option<String>`
  - `pub trait DetailFetcher { async fn fetch_detail_html(&self, mikan_id: &str) -> Result<String, String>; }`
  - `pub struct IdentityResolver<F: DetailFetcher>`，含
    `pub async fn resolve(&self, mikan_id: &str) -> Vec<ExternalId>`

- [ ] **Step 1: 寫失敗測試**

建立 `fetchers/mikanani/src/identity_resolver.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeFetcher {
        html: String,
        calls: AtomicUsize,
    }

    impl FakeFetcher {
        fn new(html: &str) -> Self {
            Self { html: html.to_string(), calls: AtomicUsize::new(0) }
        }
    }

    impl DetailFetcher for FakeFetcher {
        async fn fetch_detail_html(&self, _mikan_id: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.html.clone())
        }
    }

    #[test]
    fn extracts_mikan_id_from_a_bangumi_rss_url() {
        assert_eq!(
            mikan_id_from_rss_url("https://mikanani.me/RSS/Bangumi?bangumiId=3822&subgroupid=1236"),
            Some("3822".to_string())
        );
    }

    #[test]
    fn returns_none_for_a_feed_that_mixes_many_anime() {
        assert_eq!(mikan_id_from_rss_url("https://mikanani.me/RSS/MyBangumi?token=abc"), None);
    }

    #[tokio::test]
    async fn reports_both_the_mikan_and_bgm_identities() {
        let f = FakeFetcher::new(r#"<a href="https://bgm.tv/subject/548818">x</a>"#);
        let r = IdentityResolver::new(f);
        let ids = r.resolve("3822").await;
        assert_eq!(
            ids,
            vec![ExternalId::new("mikan", "3822"), ExternalId::new("bgm", "548818")]
        );
    }

    #[tokio::test]
    async fn still_reports_the_mikan_identity_when_no_bgm_link_exists() {
        let f = FakeFetcher::new("<p>no link here</p>");
        let r = IdentityResolver::new(f);
        assert_eq!(r.resolve("3822").await, vec![ExternalId::new("mikan", "3822")]);
    }

    #[tokio::test]
    async fn caches_by_mikan_id_so_a_feed_costs_one_fetch() {
        let f = FakeFetcher::new(r#"<a href="https://bgm.tv/subject/548818">x</a>"#);
        let r = IdentityResolver::new(f);
        r.resolve("3822").await;
        r.resolve("3822").await;
        r.resolve("3822").await;
        assert_eq!(r.fetcher().calls.load(Ordering::SeqCst), 1);
    }
}
```

- [ ] **Step 2: 執行測試確認失敗**

在 `fetchers/mikanani/src/lib.rs` 加入 `pub mod identity_resolver;`

Run: `cargo test -p mikanani-fetcher identity_resolver 2>&1 | tail -5`
Expected: FAIL，`cannot find type IdentityResolver`

- [ ] **Step 3: 實作**

在 `identity_resolver.rs` 測試模組上方加入：

```rust
use crate::detail_scraper::parse_bgm_subject_id;
use regex::Regex;
use shared::ExternalId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// 從 mikan 的單作品 RSS URL 抽出 mikan 的 bangumiId。
///
/// 只有 `RSS/Bangumi?bangumiId=` 這種單作品 feed 能在訂閱層取得身分；
/// `RSS/MyBangumi` 這類混合 feed 回傳 None，需逐 item 反查（見 fetch_task）。
pub fn mikan_id_from_rss_url(rss_url: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[?&]bangumiId=(\d+)").unwrap());
    re.captures(rss_url).map(|c| c[1].to_string())
}

/// 取得 mikan detail 頁 HTML 的能力。抽成 trait 以便測試不打網路。
pub trait DetailFetcher {
    fn fetch_detail_html(
        &self,
        mikan_id: &str,
    ) -> impl std::future::Future<Output = Result<String, String>> + Send;
}

/// 把 mikan id 解析成一組 external id。
///
/// 以 mikan id 為鍵快取：一個 feed 內數十個 item 共用同一部作品，
/// 沒有快取會對 mikan 重複打數十次。
pub struct IdentityResolver<F: DetailFetcher> {
    fetcher: F,
    cache: Mutex<HashMap<String, Vec<ExternalId>>>,
}

impl<F: DetailFetcher> IdentityResolver<F> {
    pub fn new(fetcher: F) -> Self {
        Self { fetcher, cache: Mutex::new(HashMap::new()) }
    }

    pub fn fetcher(&self) -> &F {
        &self.fetcher
    }

    /// 回報這個 mikan id 對應的全部身分。
    ///
    /// mikan 身分永遠可得；bgm 身分取決於 detail 頁是否登載。
    /// detail 頁抓取失敗時只回 mikan 身分——身分不完整優於身分錯誤。
    pub async fn resolve(&self, mikan_id: &str) -> Vec<ExternalId> {
        if let Some(hit) = self.cache.lock().unwrap().get(mikan_id) {
            return hit.clone();
        }

        let mut ids = vec![ExternalId::new("mikan", mikan_id)];
        match self.fetcher.fetch_detail_html(mikan_id).await {
            Ok(html) => {
                if let Some(bgm) = parse_bgm_subject_id(&html) {
                    ids.push(ExternalId::new("bgm", bgm));
                } else {
                    tracing::warn!("mikan {} detail page has no bgm.tv link", mikan_id);
                }
            }
            Err(e) => tracing::warn!("failed to fetch mikan {} detail page: {}", mikan_id, e),
        }

        self.cache.lock().unwrap().insert(mikan_id.to_string(), ids.clone());
        ids
    }
}
```

- [ ] **Step 4: 執行測試確認通過**

Run: `cargo test -p mikanani-fetcher identity_resolver 2>&1 | tail -5`
Expected: PASS（5 passed）

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p mikanani-fetcher -- -D warnings
git add fetchers/mikanani/
git commit -m "feat: resolve mikan subscriptions to their external identities

All 12 prod subscriptions carry a mikan id in their RSS url, so identity is
knowable before a single item is parsed. Cache by mikan id: one feed carries
dozens of items for the same season.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: mikanani — 把 external ids 掛上 raw items

**Files:**
- Modify: `fetchers/mikanani/src/fetch_task.rs:43-72`（`execute`）
- Modify: `fetchers/mikanani/src/main.rs`（建構 `IdentityResolver` 並注入）
- Test: `fetchers/mikanani/src/fetch_task.rs` 的 `mod tests`

**Interfaces:**
- Consumes: `IdentityResolver`、`mikan_id_from_rss_url`（Task 3）
- Produces: `execute` 回傳的 `Vec<RawAnimeItem>` 其 `external_ids` 已填。

- [ ] **Step 1: 讀懂現有 execute**

Run: `sed -n '30,75p' fetchers/mikanani/src/fetch_task.rs`
記下 `execute` 的簽章與它如何取得 `rss_url`。若簽章與下方步驟不符，以實際簽章為準調整。

- [ ] **Step 2: 寫失敗測試**

在 `fetch_task.rs` 的 `mod tests` 加入：

```rust
    #[tokio::test]
    async fn stamps_every_item_with_the_subscription_identity() {
        // 單作品 feed：身分來自訂閱 URL，全部 item 共用。
        let items = attach_external_ids(
            vec![
                ExternalId::new("mikan", "3822"),
                ExternalId::new("bgm", "548818"),
            ],
            vec![
                RawAnimeItem { title: "ep1".into(), description: None, download_url: "u1".into(), pub_date: None, external_ids: vec![] },
                RawAnimeItem { title: "ep2".into(), description: None, download_url: "u2".into(), pub_date: None, external_ids: vec![] },
            ],
        );

        assert_eq!(items.len(), 2);
        for item in &items {
            assert_eq!(
                item.external_ids,
                vec![ExternalId::new("mikan", "3822"), ExternalId::new("bgm", "548818")]
            );
        }
    }

    #[tokio::test]
    async fn leaves_items_untouched_when_identity_is_unknown() {
        let items = attach_external_ids(
            vec![],
            vec![RawAnimeItem { title: "ep1".into(), description: None, download_url: "u1".into(), pub_date: None, external_ids: vec![] }],
        );
        assert!(items[0].external_ids.is_empty());
    }
```

- [ ] **Step 3: 執行測試確認失敗**

Run: `cargo test -p mikanani-fetcher attach_external_ids 2>&1 | tail -5`
Expected: FAIL，`cannot find function attach_external_ids`

- [ ] **Step 4: 實作純函式**

在 `fetch_task.rs` 加入：

```rust
/// 把訂閱層解析出的身分蓋到該 feed 的每個 item 上。
///
/// 抽成純函式以便獨立測試：身分解析（打網路）與身分套用（純資料）分開。
pub fn attach_external_ids(
    ids: Vec<shared::ExternalId>,
    items: Vec<RawAnimeItem>,
) -> Vec<RawAnimeItem> {
    items
        .into_iter()
        .map(|item| RawAnimeItem { external_ids: ids.clone(), ..item })
        .collect()
}
```

- [ ] **Step 5: 執行測試確認通過**

Run: `cargo test -p mikanani-fetcher attach_external_ids 2>&1 | tail -5`
Expected: PASS（2 passed）

- [ ] **Step 6: 接進 execute**

在 `execute` 內取得 raw items 之後、回傳之前插入：

```rust
        // 身分優先取自訂閱 URL（單作品 feed，12/12 產品訂閱屬此類）。
        // 混合 feed 取不到 mikan id，此時 items 不帶身分，由 core 送入待認領佇列。
        let items = match crate::identity_resolver::mikan_id_from_rss_url(rss_url) {
            Some(mikan_id) => {
                let ids = self.identity.resolve(&mikan_id).await;
                attach_external_ids(ids, items)
            }
            None => {
                tracing::warn!("no mikan id in rss url {}, items will carry no identity", rss_url);
                items
            }
        };
```

並在 `FetchTask` struct 加入欄位 `identity: Arc<IdentityResolver<RealDetailFetcher>>`，
於 `new()` 接收。`RealDetailFetcher` 在 `main.rs` 以既有 `RealDetailScraper` 的 HTTP client
實作 `DetailFetcher`（打 `https://mikanani.me/Home/Bangumi/{id}`）。

- [ ] **Step 7: 全 package 測試**

Run: `cargo test -p mikanani-fetcher 2>&1 | tail -10`
Expected: 全數 PASS

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy -p mikanani-fetcher -- -D warnings
git add fetchers/mikanani/
git commit -m "feat: report subscription identity on every raw item

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: core — 資料庫遷移

**Files:**
- Create: `core-service/migrations/2026-07-16-000000-work-external-identity/{up,down}.sql`

**Interfaces:**
- Produces: 表 `anime_external_ids`、`metadata_namespaces`、`pending_identities`；
  欄位 `raw_anime_items.external_ids`、`animes.title`、
  `anime_works.is_active` / `soft_deleted_at`；`anime_cover_images.anime_id`。

- [ ] **Step 1: 產生遷移骨架**

```bash
cd core-service && diesel migration generate work_external_identity
```

- [ ] **Step 2: 寫 up.sql**

```sql
-- 作品身分過去由 AI 從單一 RSS 標題發明成 static 字串（title_parsers.anime_title_source
-- = 'static'），metadata service 再拿該字串去 bgm.tv 模糊搜尋並盲取 list[0]。產品環境
-- 因此出現「てんびん」這種與作品完全無關的季別，以及同一部戲的三個重複 work。
-- 改以 fetcher 回報的 namespaced external id 作為身分依據。

-- 一季在某外部站台的身分。bgm subject 是季別層級的（548818 = 金牌得主 第二季），
-- 故掛在 animes 而非 anime_works（後者是系列層）。
CREATE TABLE anime_external_ids (
    external_ref_id SERIAL PRIMARY KEY,
    anime_id        INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
    namespace       VARCHAR(50) NOT NULL,
    external_id     VARCHAR(255) NOT NULL,
    source          VARCHAR(20) NOT NULL DEFAULT 'fetcher',
    created_at      TIMESTAMP NOT NULL DEFAULT now(),
    CONSTRAINT anime_external_ids_source_check CHECK (source IN ('fetcher', 'manual')),
    -- 樞紐約束：同一個 bgm id 不可能長出兩季。這讓「欺诈游戏 / LIAR GAME /
    -- 欺诈游戏 三個 work」在資料庫層變成不可能，而非靠應用層自律。
    CONSTRAINT anime_external_ids_namespace_id_key UNIQUE (namespace, external_id),
    -- 一季在一個 namespace 只有一個 id。
    CONSTRAINT anime_external_ids_anime_namespace_key UNIQUE (anime_id, namespace)
);

CREATE INDEX idx_anime_external_ids_anime ON anime_external_ids(anime_id);

-- metadata service 註冊時聲明它認領哪些 namespace。core 據此過濾 fetcher 回報的身分。
CREATE TABLE metadata_namespaces (
    module_id  INTEGER NOT NULL REFERENCES service_modules(module_id) ON DELETE CASCADE,
    namespace  VARCHAR(50) NOT NULL,
    priority   INTEGER NOT NULL DEFAULT 50,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY (module_id, namespace)
);

-- 身分無法確定解析時的待認領佇列。搜尋只能建議，永遠不能自己寫入。
CREATE TABLE pending_identities (
    id                   SERIAL PRIMARY KEY,
    raw_item_id          INTEGER REFERENCES raw_anime_items(item_id) ON DELETE CASCADE,
    subscription_id      INTEGER REFERENCES subscriptions(subscription_id) ON DELETE SET NULL,
    source_title         TEXT NOT NULL,
    status               VARCHAR(20) NOT NULL DEFAULT 'pending',
    resolved_namespace   VARCHAR(50),
    resolved_external_id VARCHAR(255),
    created_at           TIMESTAMP NOT NULL DEFAULT now(),
    updated_at           TIMESTAMP NOT NULL DEFAULT now(),
    CONSTRAINT pending_identities_status_check
        CHECK (status IN ('pending', 'resolved', 'skipped'))
);

CREATE INDEX idx_pending_identities_status ON pending_identities(status);

-- fetcher 回報的身分需保留在 raw item 上，否則日後 reparse 會失去身分來源。
ALTER TABLE raw_anime_items
    ADD COLUMN external_ids TEXT[] NOT NULL DEFAULT '{}';

-- 季名（例如「金牌得主 第二季」），來自 metadata service。
-- anime_works.title 維持系列層（「金牌得主」）。
ALTER TABLE animes
    ADD COLUMN title VARCHAR(255);

-- anime_works 目前完全沒有 soft delete，回填時無法安全隱藏 39 個解析殘骸。
ALTER TABLE anime_works
    ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN soft_deleted_at TIMESTAMP;

-- 封面是季別的：金牌得主 S1 與 S2 的封面不同。既有資料 work:anime 為 1:1，
-- 故可安全地依該對應搬遷。
ALTER TABLE anime_cover_images
    ADD COLUMN anime_id INTEGER REFERENCES animes(anime_id) ON DELETE CASCADE;

UPDATE anime_cover_images c
SET anime_id = a.anime_id
FROM animes a
WHERE a.work_id = c.work_id;

-- 搬不過去的（work 沒有任何 anime）是殘骸，直接刪除封面記錄。
DELETE FROM anime_cover_images WHERE anime_id IS NULL;

ALTER TABLE anime_cover_images
    ALTER COLUMN anime_id SET NOT NULL,
    DROP COLUMN work_id;
```

- [ ] **Step 3: 寫 down.sql**

```sql
ALTER TABLE anime_cover_images ADD COLUMN work_id INTEGER REFERENCES anime_works(work_id) ON DELETE CASCADE;
UPDATE anime_cover_images c SET work_id = a.work_id FROM animes a WHERE a.anime_id = c.anime_id;
DELETE FROM anime_cover_images WHERE work_id IS NULL;
ALTER TABLE anime_cover_images ALTER COLUMN work_id SET NOT NULL, DROP COLUMN anime_id;

ALTER TABLE anime_works DROP COLUMN is_active, DROP COLUMN soft_deleted_at;
ALTER TABLE animes DROP COLUMN title;
ALTER TABLE raw_anime_items DROP COLUMN external_ids;

DROP TABLE pending_identities;
DROP TABLE metadata_namespaces;
DROP TABLE anime_external_ids;
```

- [ ] **Step 4: 執行並測試可逆性**

```bash
docker-compose -f docker-compose.dev.yaml up -d
cd core-service
diesel migration run
diesel migration redo
```
Expected: 兩者皆無錯誤；`schema.rs` 被自動更新（含 `anime_external_ids` 等新表）。

- [ ] **Step 5: 驗證樞紐約束確實生效**

```bash
docker exec -i <dev-postgres> psql -U postgres -d bangumi <<'SQL'
INSERT INTO anime_works (work_id, title, created_at, updated_at) VALUES (900, 'W', now(), now());
INSERT INTO seasons (season_id, year, season, created_at) VALUES (900, 2026, 'winter', now());
INSERT INTO animes (anime_id, work_id, series_no, season_id, created_at, updated_at) VALUES (900, 900, 1, 900, now(), now());
INSERT INTO animes (anime_id, work_id, series_no, season_id, created_at, updated_at) VALUES (901, 900, 2, 900, now(), now());
INSERT INTO anime_external_ids (anime_id, namespace, external_id) VALUES (900, 'bgm', '548818');
-- 這一行必須失敗：同一個 bgm id 不可對到第二季
INSERT INTO anime_external_ids (anime_id, namespace, external_id) VALUES (901, 'bgm', '548818');
SQL
```
Expected: 最後一行報 `duplicate key value violates unique constraint "anime_external_ids_namespace_id_key"`

- [ ] **Step 6: Commit**

```bash
git add core-service/migrations/ core-service/src/schema.rs
git commit -m "feat: make duplicate work identities impossible at the schema level

UNIQUE (namespace, external_id) is the hinge of the design: prod currently
has one anime spread across three works because identity was a title string.
Enforce convergence in the database rather than trusting the app layer.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: core — metadata namespace 註冊落地

**Files:**
- Modify: `core-service/src/handlers/services.rs:78-106`（新增 metadata 分支）、
  新增 `fn save_metadata_namespaces`（比照 `save_downloader_capabilities:183`）

**Interfaces:**
- Consumes: `shared::Capabilities.namespaces`（Task 1）、`metadata_namespaces` 表（Task 5）
- Produces: metadata service 註冊後，`metadata_namespaces` 有對應列。

- [ ] **Step 1: 實作 save_metadata_namespaces**

在 `services.rs` 的 `save_downloader_capabilities` 之後加入：

```rust
/// 記錄 metadata service 認領的 namespace。
///
/// 比照 downloader capabilities：先刪後插，讓服務重啟時的宣告永遠是最新的。
fn save_metadata_namespaces(
    conn: &mut diesel::PgConnection,
    service_name: &str,
    namespaces: &[String],
) {
    use crate::schema::{metadata_namespaces, service_modules};

    let module_id: Option<i32> = service_modules::table
        .filter(service_modules::name.eq(service_name))
        .select(service_modules::module_id)
        .first::<i32>(conn)
        .ok();

    let Some(module_id) = module_id else {
        tracing::error!("Could not find module_id for service: {}", service_name);
        return;
    };

    if let Err(e) = diesel::delete(
        metadata_namespaces::table.filter(metadata_namespaces::module_id.eq(module_id)),
    )
    .execute(conn)
    {
        tracing::error!("Failed to clear metadata namespaces: {}", e);
        return;
    }

    for ns in namespaces {
        if let Err(e) = diesel::insert_into(metadata_namespaces::table)
            .values((
                metadata_namespaces::module_id.eq(module_id),
                metadata_namespaces::namespace.eq(ns),
            ))
            .execute(conn)
        {
            tracing::error!("Failed to register namespace {}: {}", ns, e);
        }
    }

    tracing::info!(
        "Metadata service {} claims namespaces: {:?}",
        service_name,
        namespaces
    );
}
```

- [ ] **Step 2: 接進 register**

在 `services.rs` 的 Viewer 分支（約 line 109）之前加入：

```rust
                    if payload.service_type == ServiceType::Metadata
                        && !payload.capabilities.namespaces.is_empty()
                    {
                        save_metadata_namespaces(
                            &mut conn,
                            &payload.service_name,
                            &payload.capabilities.namespaces,
                        );
                    }
```

- [ ] **Step 3: metadata service 宣告 namespace**

`metadata/src/main.rs:63-70` 的 `Capabilities` 建構改為：

```rust
            capabilities: shared::Capabilities {
                fetch_endpoint: None,
                search_endpoint: None,
                detail_endpoint: None,
                download_endpoint: None,
                sync_endpoint: None,
                supported_download_types: vec![],
                // 本服務解析 bgm.tv 的 subject id。
                namespaces: vec!["bgm".to_string()],
            },
```

- [ ] **Step 4: 驗證**

```bash
cargo run --bin core-service &
cargo run --bin metadata &
sleep 5
docker exec -i <dev-postgres> psql -U postgres -d bangumi -c "SELECT * FROM metadata_namespaces;"
```
Expected: 一列 `namespace = bgm`

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace -- -D warnings
git add core-service/src/handlers/services.rs metadata/src/main.rs
git commit -m "feat: let metadata services declare the namespace they resolve

Routing identity by namespace is what keeps a future metadata swap from
requiring a rewash of existing rows.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: metadata — enrich 改吃 external id，刪除 list[0] 盲選

**Files:**
- Modify: `metadata/src/bangumi_client.rs:34-47`（刪除 `search_anime`）、
  `metadata/src/models.rs`、`metadata/src/handlers.rs`、`metadata/src/main.rs`（路由）

**Interfaces:**
- Consumes: 無
- Produces:
  - `POST /enrich/anime { namespace, external_id }` → `{ title, title_cn, summary, air_date, end_date, cover_images }`
  - `POST /enrich/episodes { namespace, external_id, episode_no }`
  - `GET /search/candidates?q=` → `{ candidates: [{ external_id, title, air_date, image }] }`
  - `BangumiClient::get_subject(&self, subject_id: &str) -> Result<Option<SubjectMeta>>`
  - `BangumiClient::search_candidates(&self, q: &str, limit: usize) -> Result<Vec<Candidate>>`

- [ ] **Step 1: 刪除盲選路徑**

刪除 `metadata/src/bangumi_client.rs:33-47` 的 `search_anime`。它是撈錯作品的直接成因：

```rust
        let id = body["list"][0]["id"].as_i64().map(|v| v as i32);   // 零驗證
```

- [ ] **Step 2: 寫失敗測試**

`metadata/src/handlers.rs` 的 `mod tests`：

```rust
    #[test]
    fn rejects_a_namespace_this_service_does_not_own() {
        let req = EnrichAnimeRequest {
            namespace: "tmdb".into(),
            external_id: "1234".into(),
        };
        assert!(!owns_namespace(&req.namespace));
    }

    #[test]
    fn accepts_its_own_namespace() {
        assert!(owns_namespace("bgm"));
    }
```

- [ ] **Step 3: 執行測試確認失敗**

Run: `cargo test -p metadata owns_namespace 2>&1 | tail -5`
Expected: FAIL，`cannot find function owns_namespace`

- [ ] **Step 4: 實作**

`metadata/src/models.rs`：

```rust
pub const OWNED_NAMESPACE: &str = "bgm";

#[derive(Debug, Clone, Deserialize)]
pub struct EnrichAnimeRequest {
    pub namespace: String,
    pub external_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnrichEpisodesRequest {
    pub namespace: String,
    pub external_id: String,
    pub episode_no: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnrichAnimeResponse {
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    pub cover_images: Vec<CoverImageInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub external_id: String,
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidatesResponse {
    pub candidates: Vec<Candidate>,
}
```

`metadata/src/handlers.rs`：

```rust
/// 本服務只解析 bgm 的 subject id。收到別的 namespace 一律拒絕，
/// 不猜、不 fallback——猜測正是這次要根除的東西。
pub fn owns_namespace(namespace: &str) -> bool {
    namespace == crate::models::OWNED_NAMESPACE
}

pub async fn enrich_anime(
    State(state): State<AppState>,
    Json(req): Json<EnrichAnimeRequest>,
) -> impl IntoResponse {
    if !owns_namespace(&req.namespace) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "unsupported_namespace", "message":
                format!("this service resolves '{}', not '{}'", OWNED_NAMESPACE, req.namespace) })),
        )
            .into_response();
    }

    match state.bangumi.get_subject(&req.external_id).await {
        Ok(Some(meta)) => Json(EnrichAnimeResponse {
            title: meta.title,
            title_cn: meta.title_cn,
            summary: meta.summary,
            air_date: meta.air_date,
            cover_images: meta.cover_images,
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("get_subject({}) failed: {}", req.external_id, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// 只建議，永不寫入。供待認領佇列 UI 呈現候選。
pub async fn search_candidates(
    State(state): State<AppState>,
    Query(q): Query<CandidatesQuery>,
) -> impl IntoResponse {
    match state.bangumi.search_candidates(&q.q, 5).await {
        Ok(candidates) => Json(CandidatesResponse { candidates }).into_response(),
        Err(e) => {
            tracing::error!("search_candidates({}) failed: {}", q.q, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
```

`bangumi_client.rs` 新增（合併原本三個各打一次 `/v0/subjects/{id}` 的方法為一次）：

```rust
pub struct SubjectMeta {
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub summary: Option<String>,
    pub air_date: Option<String>,
    pub cover_images: Vec<CoverImageInfo>,
}

impl BangumiClient {
    /// 依 subject id 取得權威資料。一次請求取回全部欄位——
    /// 原本 get_cover_images / get_subject_meta 各打一次同一個端點。
    pub async fn get_subject(&self, subject_id: &str) -> Result<Option<SubjectMeta>> {
        let url = format!("{}/v0/subjects/{}", BANGUMI_API_BASE, subject_id);
        let resp = self.http.get(&url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("bgm returned {}", resp.status()));
        }
        let body: serde_json::Value = resp.json().await?;

        let cover_images = body["images"]["large"]
            .as_str()
            .filter(|u| !u.is_empty() && !u.ends_with("no_img.gif"))
            .map(|u| vec![CoverImageInfo { url: u.to_string(), source: "bangumi".to_string() }])
            .unwrap_or_default();

        Ok(Some(SubjectMeta {
            title: body["name"].as_str().map(str::to_string),
            title_cn: body["name_cn"].as_str().map(str::to_string),
            summary: body["summary"].as_str().map(str::to_string),
            air_date: body["date"].as_str().map(str::to_string),
            cover_images,
        }))
    }

    /// 模糊搜尋，只用於產生候選建議。
    pub async fn search_candidates(&self, q: &str, limit: usize) -> Result<Vec<Candidate>> {
        let url = format!(
            "{}/search/subject/{}?type=2&responseGroup=small&max_results={}",
            BANGUMI_API_BASE,
            urlencoding::encode(q),
            limit
        );
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(body["list"]
            .as_array()
            .map(|list| {
                list.iter()
                    .take(limit)
                    .filter_map(|s| {
                        Some(Candidate {
                            external_id: s["id"].as_i64()?.to_string(),
                            title: s["name"].as_str().map(str::to_string),
                            title_cn: s["name_cn"].as_str().map(str::to_string),
                            air_date: s["air_date"].as_str().map(str::to_string),
                            image: s["images"]["grid"].as_str().map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}
```

`get_episode` 的 `bangumi_id: i32` 改為 `subject_id: &str`。
`main.rs` 加路由：`.route("/search/candidates", get(handlers::search_candidates))`

- [ ] **Step 5: 執行測試確認通過**

Run: `cargo test -p metadata 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 6: 對真實 bgm API 驗證**

```bash
cargo run --bin metadata &
sleep 3
curl -s -X POST localhost:8004/enrich/anime -H 'Content-Type: application/json' \
  -d '{"namespace":"bgm","external_id":"456080"}' | python3 -m json.tool | head -8
```
Expected: `title_cn` 為「转学后班上的清纯可爱美少女，竟是小时候玩在一起的哥儿们」
（即產品環境被 AI 錯標為「てんびん」的那一季）。

```bash
curl -s -X POST localhost:8004/enrich/anime -H 'Content-Type: application/json' \
  -d '{"namespace":"tmdb","external_id":"1"}' -o /dev/null -w '%{http_code}\n'
```
Expected: `400`

（port 以 `metadata/src/main.rs` 實際綁定為準。）

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy -p metadata -- -D warnings
git add metadata/
git commit -m "feat: resolve metadata by subject id instead of guessing from a title

search_anime took list[0] of a fuzzy search over an AI-invented title with
zero validation. Delete that path: enrich now requires an external id, and
fuzzy search survives only as a suggestion endpoint a human confirms.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 8: core — 身分解析服務

**Files:**
- Create: `core-service/src/services/identity.rs`
- Modify: `core-service/src/services/mod.rs`

**Interfaces:**
- Consumes: `anime_external_ids` / `metadata_namespaces`（Task 5）、metadata `/enrich/anime`（Task 7）
- Produces:
  - `pub fn registered_namespaces(conn: &mut PgConnection) -> Result<Vec<String>, String>`
  - `pub fn usable_ids(all: &[ExternalId], registered: &[String]) -> Vec<ExternalId>`
  - `pub fn find_anime_by_external_id(conn, &ExternalId) -> Result<Option<i32>, String>`
  - `pub fn season_name_from_date(date: &str) -> (i32, &'static str)`
  - `pub fn link_external_id(conn, anime_id: i32, &ExternalId, source: &str) -> Result<(), String>`

- [ ] **Step 1: 寫失敗測試**

建立 `core-service/src/services/identity.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::ExternalId;

    #[test]
    fn keeps_only_ids_a_registered_service_can_resolve() {
        let all = vec![ExternalId::new("mikan", "3822"), ExternalId::new("bgm", "548818")];
        let usable = usable_ids(&all, &["bgm".to_string()]);
        // mikan 身分僅供追溯，沒有 metadata service 認領它。
        assert_eq!(usable, vec![ExternalId::new("bgm", "548818")]);
    }

    #[test]
    fn yields_nothing_when_no_service_claims_any_namespace() {
        let all = vec![ExternalId::new("mikan", "3822")];
        assert!(usable_ids(&all, &["bgm".to_string()]).is_empty());
    }

    #[test]
    fn derives_year_and_season_from_an_air_date() {
        assert_eq!(season_name_from_date("2026-01-24"), (2026, "winter"));
        assert_eq!(season_name_from_date("2026-04-06"), (2026, "spring"));
        assert_eq!(season_name_from_date("2026-07-09"), (2026, "summer"));
        assert_eq!(season_name_from_date("2026-10-01"), (2026, "autumn"));
    }

    #[test]
    fn falls_back_to_unknown_for_an_unparsable_date() {
        // aired_date 在產品環境全為 NULL；解析不出來時不要編一個年份。
        assert_eq!(season_name_from_date(""), (0, "unknown"));
        assert_eq!(season_name_from_date("not-a-date"), (0, "unknown"));
    }
}
```

- [ ] **Step 2: 執行測試確認失敗**

在 `core-service/src/services/mod.rs` 加 `pub mod identity;`

Run: `cargo test -p core-service identity:: 2>&1 | tail -5`
Expected: FAIL，`cannot find function usable_ids`

- [ ] **Step 3: 實作**

```rust
use crate::schema::{anime_external_ids, metadata_namespaces};
use diesel::prelude::*;
use shared::ExternalId;

/// 目前有 metadata service 認領的 namespace。
pub fn registered_namespaces(conn: &mut PgConnection) -> Result<Vec<String>, String> {
    metadata_namespaces::table
        .select(metadata_namespaces::namespace)
        .distinct()
        .load::<String>(conn)
        .map_err(|e| format!("Failed to load metadata namespaces: {}", e))
}

/// 從 fetcher 回報的全部身分中，濾出有服務能解析的那些。
///
/// fetcher 回報它所知道的一切（含 mikan/3822 這種僅供追溯的身分）；
/// 只有被認領的 namespace 才能用來決定作品身分。
pub fn usable_ids(all: &[ExternalId], registered: &[String]) -> Vec<ExternalId> {
    all.iter()
        .filter(|e| registered.iter().any(|ns| ns == &e.namespace))
        .cloned()
        .collect()
}

/// 依 external id 找出已存在的季。
pub fn find_anime_by_external_id(
    conn: &mut PgConnection,
    external: &ExternalId,
) -> Result<Option<i32>, String> {
    anime_external_ids::table
        .filter(anime_external_ids::namespace.eq(&external.namespace))
        .filter(anime_external_ids::external_id.eq(&external.id))
        .select(anime_external_ids::anime_id)
        .first::<i32>(conn)
        .optional()
        .map_err(|e| format!("Failed to look up external id {}: {}", external, e))
}

/// 把身分綁到季上。人工修正（manual）永遠不被 fetcher 覆寫。
pub fn link_external_id(
    conn: &mut PgConnection,
    anime_id: i32,
    external: &ExternalId,
    source: &str,
) -> Result<(), String> {
    diesel::insert_into(anime_external_ids::table)
        .values((
            anime_external_ids::anime_id.eq(anime_id),
            anime_external_ids::namespace.eq(&external.namespace),
            anime_external_ids::external_id.eq(&external.id),
            anime_external_ids::source.eq(source),
        ))
        .on_conflict((anime_external_ids::anime_id, anime_external_ids::namespace))
        .do_update()
        .set(anime_external_ids::external_id.eq(&external.id))
        .filter(anime_external_ids::source.ne("manual"))
        .execute(conn)
        .map(|_| ())
        .map_err(|e| format!("Failed to link external id {}: {}", external, e))
}

/// 從 bgm 的 air date 推導年份與季別。
///
/// 產品環境的 animes.aired_date 全為 NULL、season_id 全指向 2025/unknown，
/// 但那些是 2026 年番。權威日期一到手就能同時修好這兩者。
pub fn season_name_from_date(date: &str) -> (i32, &'static str) {
    let mut parts = date.split('-');
    let (Some(y), Some(m)) = (parts.next(), parts.next()) else {
        return (0, "unknown");
    };
    let (Ok(year), Ok(month)) = (y.parse::<i32>(), m.parse::<u32>()) else {
        return (0, "unknown");
    };
    let season = match month {
        1..=3 => "winter",
        4..=6 => "spring",
        7..=9 => "summer",
        10..=12 => "autumn",
        _ => return (0, "unknown"),
    };
    (year, season)
}
```

- [ ] **Step 4: 執行測試確認通過**

Run: `cargo test -p core-service identity:: 2>&1 | tail -5`
Expected: PASS（4 passed）

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p core-service -- -D warnings
git add core-service/src/services/
git commit -m "feat: resolve work identity from external ids

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 9: core — 攝入時保留 external ids

**Files:**
- Modify: `core-service/src/handlers/fetcher_results.rs:477`（`receive_raw_fetcher_results`）、
  `core-service/src/models/db.rs:630-646`（`RawAnimeItem` / `NewRawAnimeItem`）

**Interfaces:**
- Consumes: `shared::RawAnimeItem.external_ids`（Task 1）、`raw_anime_items.external_ids`（Task 5）
- Produces: DB model `RawAnimeItem.external_ids: Vec<Option<String>>`（Diesel `Array<Nullable<Text>>` 的預設映射）

- [ ] **Step 1: 擴充 DB model**

`core-service/src/models/db.rs` 的 `RawAnimeItem` 加入欄位（順序必須與 `schema.rs` 一致）：

```rust
    /// fetcher 回報的身分，形如 ["mikan/3822", "bgm/548818"]。
    /// 保留在 raw item 上，reparse 才不會失去身分來源。
    pub external_ids: Vec<Option<String>>,
```

`NewRawAnimeItem` 同樣加入 `pub external_ids: Vec<String>,`

- [ ] **Step 2: 攝入時寫入**

在 `receive_raw_fetcher_results` 建構 `NewRawAnimeItem` 之處加入：

```rust
            external_ids: item.external_ids.iter().map(|e| e.to_string()).collect(),
```

- [ ] **Step 3: 編譯**

Run: `cargo build -p core-service 2>&1 | tail -5`
Expected: 成功。若 Diesel 報欄位順序不符，以 `schema.rs` 內 `raw_anime_items` 的宣告順序為準調整 struct。

- [ ] **Step 4: 端到端驗證**

```bash
curl -s -X POST localhost:8000/fetcher-results/raw -H 'Content-Type: application/json' -d '{
  "subscription_id": 1, "fetcher_source": "mikanani", "success": true, "error_message": null,
  "items": [{"title":"[X] Y [01][1080p]","description":null,"download_url":"http://e/1.torrent",
             "pub_date":null,"external_ids":["mikan/3822","bgm/548818"]}]
}' | head -3
docker exec -i <dev-postgres> psql -U postgres -d bangumi \
  -c "SELECT item_id, external_ids FROM raw_anime_items ORDER BY item_id DESC LIMIT 1;"
```
Expected: `external_ids` 為 `{mikan/3822,bgm/548818}`

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p core-service -- -D warnings
git add core-service/src/
git commit -m "feat: persist fetcher-reported identity on raw items

Keeping identity on the raw item is what lets a later reparse reuse it
instead of falling back to guessing from the title again.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 10: core — 以 external id 決定季，parser 不再決定作品名

**Files:**
- Modify: `core-service/src/handlers/fetcher_results.rs:687-713`（`process_parsed_result` 前三步）
- Test: `core-service/tests/integration/identity_test.rs`（新）

**Interfaces:**
- Consumes: Task 8 的全部函式、Task 9 的 `RawAnimeItem.external_ids`
- Produces: `pub(crate) async fn resolve_or_create_anime(conn, raw_item, parsed, metadata_url) -> Result<Option<Anime>, String>`
  （`Ok(None)` 表示身分不明，呼叫端應送入待認領佇列）

- [ ] **Step 1: 寫失敗測試**

建立 `core-service/tests/integration/identity_test.rs`：

```rust
// 這些測試需要 DATABASE_URL 指向開發資料庫（docker-compose.dev.yaml）。
use shared::ExternalId;

#[tokio::test]
async fn two_items_with_the_same_bgm_id_land_on_one_season() {
    let mut conn = test_conn();
    let a = insert_season_with_identity(&mut conn, &ExternalId::new("bgm", "548818"));
    let b = insert_season_with_identity(&mut conn, &ExternalId::new("bgm", "548818"));
    // 這正是產品環境「欺诈游戏 / LIAR GAME / 欺诈游戏」三個 work 的成因，
    // 現在由 UNIQUE (namespace, external_id) 在資料庫層擋下。
    assert_eq!(a, b);
}

#[tokio::test]
async fn different_seasons_of_one_series_stay_distinct() {
    let mut conn = test_conn();
    let s2 = insert_season_with_identity(&mut conn, &ExternalId::new("bgm", "548818"));
    let s1 = insert_season_with_identity(&mut conn, &ExternalId::new("bgm", "389156"));
    assert_ne!(s1, s2);
}

#[tokio::test]
async fn a_manual_identity_is_not_overwritten_by_a_fetcher() {
    let mut conn = test_conn();
    let anime_id = insert_bare_season(&mut conn);
    link_external_id(&mut conn, anime_id, &ExternalId::new("bgm", "111"), "manual").unwrap();
    link_external_id(&mut conn, anime_id, &ExternalId::new("bgm", "222"), "fetcher").unwrap();
    assert_eq!(current_external_id(&mut conn, anime_id), "111");
}
```

輔助函式 `test_conn` / `insert_bare_season` / `insert_season_with_identity` /
`current_external_id` 依 `core-service/tests/` 既有慣例撰寫；
先 `rg -n 'fn test_conn|DATABASE_URL' core-service/tests/ | head` 確認是否已有可重用者。

- [ ] **Step 2: 執行測試確認失敗**

Run: `cargo test -p core-service --test integration identity 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: 實作 resolve_or_create_anime**

在 `fetcher_results.rs` 加入：

```rust
/// 決定這個 raw item 屬於哪一季。
///
/// 身分只來自 fetcher 回報的 external id——parser 不再決定作品名。
/// 產品環境曾因 parser 的 static title 把一整季標成「てんびん」。
///
/// 回傳 Ok(None) 表示身分不明，呼叫端應送入待認領佇列而非猜一個。
pub(crate) async fn resolve_or_create_anime(
    conn: &mut PgConnection,
    raw_item: &RawAnimeItem,
    parsed: &crate::services::title_parser::ParsedResult,
    metadata_url: &str,
) -> Result<Option<Anime>, String> {
    use crate::services::identity::*;

    let reported: Vec<shared::ExternalId> = raw_item
        .external_ids
        .iter()
        .flatten()
        .filter_map(|s| s.parse().ok())
        .collect();

    let registered = registered_namespaces(conn)?;
    let usable = usable_ids(&reported, &registered);

    let Some(external) = usable.first() else {
        tracing::warn!(
            "raw item {} carries no resolvable identity (reported: {:?})",
            raw_item.item_id,
            raw_item.external_ids
        );
        return Ok(None);
    };

    if let Some(anime_id) = find_anime_by_external_id(conn, external)? {
        return animes::table
            .find(anime_id)
            .first::<Anime>(conn)
            .map(Some)
            .map_err(|e| format!("Failed to load anime {}: {}", anime_id, e));
    }

    // 未知身分：向 metadata service 取權威資料後建立。
    let meta = fetch_authoritative_metadata(metadata_url, external).await?;

    // 系列層 title 取 metadata 的中文名（退回原名）；季別資訊由 parsed.series_no 提供。
    let series_title = meta.title_cn.clone().or(meta.title.clone()).ok_or_else(|| {
        format!("metadata service returned no title for {}", external)
    })?;

    let work = create_or_get_anime(conn, &series_title)?;

    let (year, season_name) = meta
        .air_date
        .as_deref()
        .map(season_name_from_date)
        .unwrap_or((0, "unknown"));
    let season = create_or_get_season(conn, year, season_name)?;

    let anime = create_or_get_series(
        conn,
        work.work_id,
        parsed.series_no,
        season.season_id,
        meta.summary.as_deref().unwrap_or(""),
    )?;

    // 季名與播出日期一律以 metadata 為準。
    diesel::update(animes::table.find(anime.anime_id))
        .set((
            animes::title.eq(meta.title_cn.as_ref().or(meta.title.as_ref())),
            animes::aired_date.eq(meta
                .air_date
                .as_deref()
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())),
        ))
        .execute(conn)
        .map_err(|e| format!("Failed to update anime metadata: {}", e))?;

    link_external_id(conn, anime.anime_id, external, "fetcher")?;

    Ok(Some(anime))
}
```

- [ ] **Step 4: 接進 process_parsed_result**

`process_parsed_result:694-713` 的前三步（`create_or_get_anime` / `create_or_get_season` /
`create_or_get_series`）以 `resolve_or_create_anime` 取代。
`process_parsed_result` 需改為 `async`，並接受 `metadata_url: &str`。
回傳 `Ok(None)` 時，呼叫端改寫 `pending_identities`（Task 11）並將 raw item 標為
`status = 'no_identity'`，**不建立 work**。

- [ ] **Step 5: 執行測試確認通過**

Run: `cargo test -p core-service --test integration identity 2>&1 | tail -5`
Expected: PASS（3 passed）

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy -p core-service -- -D warnings
git add core-service/
git commit -m "feat: take work identity away from the parser

The parser's static anime_title is what produced a season called てんびん and
one anime spread across three works. Identity now comes from the fetcher's
external id; the parser keeps episode, group and resolution.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 11: core — 待認領佇列

**Files:**
- Create: `core-service/src/handlers/pending_identities.rs`
- Modify: `core-service/src/handlers/mod.rs`、`core-service/src/main.rs`（路由）

**Interfaces:**
- Consumes: `pending_identities` 表（Task 5）、`link_external_id`（Task 8）
- Produces:
  - `GET /pending-identities` → 待認領清單
  - `GET /pending-identities/:id/candidates` → 轉呼叫 metadata `/search/candidates`
  - `POST /pending-identities/:id/resolve { namespace, external_id }` → 寫入身分並重跑該 item

- [ ] **Step 1: 寫失敗測試**

`core-service/tests/integration/pending_identity_test.rs`：

```rust
#[tokio::test]
async fn an_item_without_identity_is_queued_and_creates_no_work() {
    let mut conn = test_conn();
    let works_before = count_works(&mut conn);
    ingest_item_without_identity(&mut conn).await;
    // 沒有身分就不要發明一個——這是整個設計的重點。
    assert_eq!(count_works(&mut conn), works_before);
    assert_eq!(count_pending(&mut conn), 1);
}

#[tokio::test]
async fn resolving_a_queued_item_records_a_manual_identity() {
    let mut conn = test_conn();
    let pending_id = ingest_item_without_identity(&mut conn).await;
    resolve_pending(&mut conn, pending_id, "bgm", "548818").await;
    assert_eq!(pending_status(&mut conn, pending_id), "resolved");
    assert_eq!(external_id_source(&mut conn, "bgm", "548818"), "manual");
}
```

- [ ] **Step 2: 執行測試確認失敗**

Run: `cargo test -p core-service --test integration pending_identity 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: 實作 handler**

```rust
use crate::schema::pending_identities;
use axum::{extract::{Path, State}, http::StatusCode, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ResolveRequest {
    pub namespace: String,
    pub external_id: String,
}

#[derive(Debug, Serialize, Queryable)]
pub struct PendingIdentity {
    pub id: i32,
    pub raw_item_id: Option<i32>,
    pub subscription_id: Option<i32>,
    pub source_title: String,
    pub status: String,
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB pool error: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    match pending_identities::table
        .filter(pending_identities::status.eq("pending"))
        .select((
            pending_identities::id,
            pending_identities::raw_item_id,
            pending_identities::subscription_id,
            pending_identities::source_title,
            pending_identities::status,
        ))
        .load::<PendingIdentity>(&mut conn)
    {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => {
            tracing::error!("Failed to list pending identities: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// 人工確認身分。source 固定為 'manual'——人的決定永遠蓋過自動推導。
pub async fn resolve(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<ResolveRequest>,
) -> impl IntoResponse {
    let external = shared::ExternalId::new(&req.namespace, &req.external_id);

    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB pool error: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 該身分必須是某個已註冊 metadata service 能解析的，否則寫進去也沒人能用。
    let registered = match crate::services::identity::registered_namespaces(&mut conn) {
        Ok(ns) => ns,
        Err(e) => {
            tracing::error!("{}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    if !registered.contains(&req.namespace) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "unsupported_namespace",
                "message": format!("no metadata service claims namespace '{}'", req.namespace)
            })),
        )
            .into_response();
    }

    let raw_item_id: Option<i32> = match pending_identities::table
        .find(id)
        .select(pending_identities::raw_item_id)
        .first::<Option<i32>>(&mut conn)
    {
        Ok(v) => v,
        Err(diesel::NotFound) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("Failed to load pending identity {}: {}", id, e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Some(raw_item_id) = raw_item_id else {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "orphan_pending",
                "message": "this queue entry has no raw item to reprocess"
            })),
        )
            .into_response();
    };

    // 以使用者指定的身分重跑該 raw item：先把身分寫到 raw item 上，
    // 再走與自動路徑完全相同的解析流程（避免兩套邏輯分歧）。
    if let Err(e) = diesel::update(raw_anime_items::table.find(raw_item_id))
        .set(raw_anime_items::external_ids.eq(vec![external.to_string()]))
        .execute(&mut conn)
    {
        tracing::error!("Failed to stamp identity on raw item {}: {}", raw_item_id, e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match crate::handlers::fetcher_results::reparse_single_item(
        &mut conn,
        raw_item_id,
        &state.metadata_url,
    )
    .await
    {
        Ok(Some(anime_id)) => {
            // 覆寫為 manual：人的決定不得被後續 fetcher 蓋掉。
            if let Err(e) = crate::services::identity::link_external_id(
                &mut conn, anime_id, &external, "manual",
            ) {
                tracing::error!("{}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            let now = chrono::Utc::now().naive_utc();
            if let Err(e) = diesel::update(pending_identities::table.find(id))
                .set((
                    pending_identities::status.eq("resolved"),
                    pending_identities::resolved_namespace.eq(&req.namespace),
                    pending_identities::resolved_external_id.eq(&req.external_id),
                    pending_identities::updated_at.eq(now),
                ))
                .execute(&mut conn)
            {
                tracing::error!("Failed to mark pending {} resolved: {}", id, e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            Json(json!({ "success": true, "anime_id": anime_id })).into_response()
        }
        Ok(None) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "still_unresolved",
                "message": "the metadata service could not resolve that external id"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to reparse item {}: {}", raw_item_id, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// 轉呼叫 metadata service 的候選端點。core 不自己做模糊比對——
/// 候選只是建議，決定權在使用者。
pub async fn candidates(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB pool error: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let source_title: String = match pending_identities::table
        .find(id)
        .select(pending_identities::source_title)
        .first(&mut conn)
    {
        Ok(t) => t,
        Err(diesel::NotFound) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("Failed to load pending identity {}: {}", id, e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let url = format!("{}/search/candidates", state.metadata_url);
    match reqwest::Client::new()
        .get(&url)
        .query(&[("q", source_title.as_str())])
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(v) => Json(v).into_response(),
            Err(e) => {
                tracing::error!("Bad candidates response: {}", e);
                StatusCode::BAD_GATEWAY.into_response()
            }
        },
        Ok(r) => {
            tracing::warn!("Metadata service returned {}", r.status());
            StatusCode::BAD_GATEWAY.into_response()
        }
        Err(e) => {
            tracing::error!("Metadata service unreachable: {}", e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
```

本 handler 需要 `reparse_single_item`，於 `fetcher_results.rs` 加入
（把既有 `process_parsed_result` 的呼叫路徑包成可依 item_id 重跑的入口）：

```rust
/// 依 item_id 重跑單一 raw item 的解析。
///
/// 待認領佇列的人工確認與自動攝入走同一段邏輯，避免兩套流程分歧。
/// 回傳 Ok(None) 表示身分仍無法解析。
pub(crate) async fn reparse_single_item(
    conn: &mut PgConnection,
    item_id: i32,
    metadata_url: &str,
) -> Result<Option<i32>, String> {
    let item: RawAnimeItem = raw_anime_items::table
        .find(item_id)
        .first(conn)
        .map_err(|e| format!("Failed to load raw item {}: {}", item_id, e))?;

    let parser = TitleParserService::new();
    let parsed = match parser.parse(conn, &item.title) {
        Ok(ParseStatus::Parsed(p)) => p,
        Ok(_) => return Err(format!("raw item {} has no matching parser", item_id)),
        Err(e) => return Err(format!("Failed to parse raw item {}: {}", item_id, e)),
    };

    match resolve_or_create_anime(conn, &item, &parsed, metadata_url).await? {
        Some(anime) => {
            process_parsed_result(conn, &item, &parsed, metadata_url).await?;
            Ok(Some(anime.anime_id))
        }
        None => Ok(None),
    }
}
```

**注意：** `TitleParserService::new()` 與 `parser.parse(...)` 的實際簽章請先以
`rg -n 'impl TitleParserService' -A 20 core-service/src/services/title_parser.rs` 確認後對齊。

`main.rs` 加路由：

```rust
        .route("/pending-identities", get(handlers::pending_identities::list))
        .route("/pending-identities/:id/resolve", post(handlers::pending_identities::resolve))
        .route("/pending-identities/:id/candidates", get(handlers::pending_identities::candidates))
```

- [ ] **Step 4: 執行測試確認通過**

Run: `cargo test -p core-service --test integration pending_identity 2>&1 | tail -5`
Expected: PASS（2 passed）

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p core-service -- -D warnings
git add core-service/
git commit -m "feat: queue unresolvable items for human confirmation

Search may suggest; only a human may write. With mikan resolving 12/12 of
prod, this path should stay rare.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 12: core + viewer — 封面改掛季，修好 resync 的 external id

**Files:**
- Modify: `core-service/src/handlers/anime.rs:1068-1160`（`fetch_and_store_covers`）
- Modify: `core-service/src/services/sync_service.rs:187`
- Modify: `viewers/jellyfin/src/handlers.rs:420-426`

**Interfaces:**
- Consumes: `anime_external_ids`（Task 5）、`anime_cover_images.anime_id`（Task 5）
- Produces: `fetch_and_store_covers(db, anime_id: i32, external: ExternalId)`

- [ ] **Step 1: 改寫 fetch_and_store_covers**

簽章由 `(db, work_id: i32, anime_title: String)` 改為
`(db, anime_id: i32, external: shared::ExternalId)`。

`anime.rs:1068` 的函式簽章與 `anime.rs:1106-1111` 的請求建構改為：

```rust
pub async fn fetch_and_store_covers(
    db: crate::db::DbPool,
    anime_id: i32,
    external: shared::ExternalId,
) {
```

```rust
        let resp = client
            .post(format!("{}/enrich/anime", metadata_url))
            .json(&serde_json::json!({
                "namespace": external.namespace,
                "external_id": external.id,
            }))
            .send()
            .await;
```

`anime.rs:1140` 之後寫入 `anime_cover_images` 時，`NewAnimeCoverImage` 的
`work_id` 欄位改為 `anime_id`（欄位已於 Task 5 搬遷），
`core-service/src/models/db.rs` 的 `NewAnimeCoverImage` 同步改欄位。

兩處呼叫點 `anime.rs:49` 與 `anime.rs:515` 目前傳的是 `(wid, title_clone)`／`(id, title)`。
兩者都需先取得該季的 bgm 身分，取不到就不呼叫——沒有身分時不要退回用標題搜尋：

```rust
                if let Ok(Some(bgm)) =
                    crate::services::identity::find_external_id(&mut conn, anime_id, "bgm")
                {
                    fetch_and_store_covers(db_clone, anime_id, shared::ExternalId::new("bgm", bgm))
                        .await;
                }
```

- [ ] **Step 2: `bangumi_id: Option<i32>` → `external_id: Option<ExternalId>`**

external id 現在是字串且帶 namespace，而 `shared/src/models.rs:229` 目前是
`pub bangumi_id: Option<i32>`。整條鏈（core → viewer → metadata）需一致改型，
否則 `548818` 這種值會在 i32 與 String 之間反覆轉換，且 namespace 資訊遺失。

`shared/src/models.rs:229`（`ViewerSyncRequest` 內）改為：

```rust
    /// 這一季的外部身分，例如 bgm/548818。metadata service 依 namespace 認領。
    #[serde(default)]
    pub external_id: Option<crate::ExternalId>,
```

`ViewerResyncRequest`（`shared/src/models.rs:235` 起）加入同一欄位。

`viewers/jellyfin/src/metadata_client.rs:26-30` 的簽章改為：

```rust
    pub async fn enrich_episodes(
        &self,
        external: &shared::ExternalId,
        episode_no: i32,
    ) -> Result<Option<EpisodeInfo>> {
```

其 body 建構（`metadata_client.rs:34-37`）改為：

```rust
            .json(&serde_json::json!({
                "namespace": external.namespace,
                "external_id": external.id,
                "episode_no": episode_no
            }))
```

`viewers/jellyfin/src/handlers.rs:199` 的參數 `bangumi_id: Option<i32>` 改為
`external: Option<shared::ExternalId>`，`:208-215` 的 guard 改為：

```rust
    let Some(external) = external else {
        tracing::warn!(
            "No external id for '{}', skipping metadata generation",
            anime_title
        );
        return Ok(());
    };
```

`:234` 的呼叫改為 `metadata.enrich_episodes(&external, episode_no).await`，
`:174` 的 `req.bangumi_id` 改為 `req.external_id`。

- [ ] **Step 3: 修好 sync_service 的 external_id**

`sync_service.rs:187` 的 `bangumi_id: None` 是既有 bug：viewer 收到 None 後
（`viewers/jellyfin/src/handlers.rs:212`）直接 skip，導致 resync 路徑的單集
metadata 從未生成過。改為查表帶出真實身分：

```rust
            external_id: crate::services::identity::find_external_id(&mut conn, anime_id, "bgm")
                .ok()
                .flatten()
                .map(|id| shared::ExternalId::new("bgm", id)),
```

`viewers/jellyfin/src/handlers.rs:420-426` 的
`None, // bangumi_id not available in resync request` 改為 `req.external_id`
（該欄位已於 Step 2 加入 `ViewerResyncRequest`）。

於 `identity.rs` 補上對應函式：

```rust
/// 取得某季在指定 namespace 的 external id。
pub fn find_external_id(
    conn: &mut PgConnection,
    anime_id: i32,
    namespace: &str,
) -> Result<Option<String>, String> {
    anime_external_ids::table
        .filter(anime_external_ids::anime_id.eq(anime_id))
        .filter(anime_external_ids::namespace.eq(namespace))
        .select(anime_external_ids::external_id)
        .first::<String>(conn)
        .optional()
        .map_err(|e| format!("Failed to look up external id for anime {}: {}", anime_id, e))
}
```

`metadata/src/handlers.rs` 的 `enrich_episodes` 亦需比照 `enrich_anime`
（Task 7）先做 `owns_namespace` 檢查再查詢。

- [ ] **Step 4: 驗證**

Run: `cargo test --workspace 2>&1 | tail -10`
Expected: 全數 PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace -- -D warnings
git add core-service/ viewers/ shared/
git commit -m "fix: attach covers to seasons and give resync a real subject id

Covers differ per season, so anime_id is the right owner. sync_service also
hard-coded bangumi_id: None, so the resync path silently skipped episode
metadata generation entirely.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 13: CLI — `backfill-identity`

**Files:**
- Create: `cli/src/commands/backfill.rs`
- Modify: `cli/src/commands/mod.rs`、`cli/src/main.rs`

**Interfaces:**
- Consumes: Task 3 的 `mikan_id_from_rss_url`、Task 7 的 `/enrich/anime`、Task 8 的 identity 函式
- Produces: `bangumi-cli backfill-identity [--dry-run|--apply]`

**背景：** 產品環境唯讀，回填由維運者手動執行。實測基準（2026-07-15）：
12 個訂閱全部帶 mikan id、全部解析出 bgm id；51 個 work 中 12 個可從訂閱到達，
39 個為殘骸（37 個無 anime，2 個有 anime 但零 links、零 downloads）。

- [ ] **Step 1: 寫失敗測試**

`cli/src/commands/backfill.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_a_rename_when_metadata_disagrees_with_the_stored_title() {
        let plan = plan_backfill(
            vec![WorkRow { work_id: 50, title: "てんびん".into(), anime_id: 166 }],
            |_| Some(ResolvedMeta {
                external_id: "bgm/456080".into(),
                title: "转学后班上的清纯可爱美少女，竟是小时候玩在一起的哥儿们".into(),
                air_date: Some("2026-07-06".into()),
            }),
        );
        assert_eq!(plan.backfills.len(), 1);
        assert_eq!(plan.backfills[0].new_title, "转学后班上的清纯可爱美少女，竟是小时候玩在一起的哥儿们");
        assert_eq!(plan.backfills[0].old_title, "てんびん");
    }

    #[test]
    fn plans_soft_deletes_for_works_no_subscription_can_reach() {
        let plan = plan_backfill_with_orphans(vec![
            OrphanRow { work_id: 1, title: "[绿茶字幕组] 金牌得主 第二季 [18][1080p]".into(), links: 0, downloads: 0 },
            OrphanRow { work_id: 38, title: "我推的孩子".into(), links: 0, downloads: 0 },
        ]);
        // 判定條件是訂閱可達性，不是「有無 anime」：work 38 有 anime 但訂閱已刪。
        assert_eq!(plan.soft_deletes.len(), 2);
    }

    #[test]
    fn refuses_to_soft_delete_a_work_that_still_has_downloads() {
        let plan = plan_backfill_with_orphans(vec![
            OrphanRow { work_id: 38, title: "我推的孩子".into(), links: 3, downloads: 2 },
        ]);
        // 實測時這 39 個都是零下載；若前提改變必須擋下而非照刪。
        assert!(plan.soft_deletes.is_empty());
        assert_eq!(plan.refused.len(), 1);
    }
}
```

- [ ] **Step 2: 執行測試確認失敗**

Run: `cargo test -p bangumi-cli backfill 2>&1 | tail -5`
Expected: FAIL，`cannot find function plan_backfill`

（package 名以 `rg '^name' cli/Cargo.toml` 為準。）

- [ ] **Step 3: 實作規劃邏輯（純函式，先不碰 DB）**

```rust
/// 從產品環境讀出的一列 work（可從訂閱到達者）。
#[derive(Debug, Clone)]
pub struct WorkRow {
    pub work_id: i32,
    pub title: String,
    pub anime_id: i32,
}

/// 無法從任何訂閱到達的 work，以及它底下還剩什麼。
#[derive(Debug, Clone)]
pub struct OrphanRow {
    pub work_id: i32,
    pub title: String,
    pub links: i64,
    pub downloads: i64,
}

/// metadata service 對某個 external id 回覆的權威資料。
#[derive(Debug, Clone)]
pub struct ResolvedMeta {
    pub external_id: String,
    pub title: String,
    pub air_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Backfill {
    pub work_id: i32,
    pub anime_id: i32,
    pub external_id: String,
    pub old_title: String,
    pub new_title: String,
    pub air_date: Option<String>,
}

/// 回填規劃：把「決定要做什麼」與「實際寫入」分開，
/// 讓 dry-run 報表與 --apply 走的是同一段邏輯。
#[derive(Debug, Default)]
pub struct BackfillPlan {
    pub backfills: Vec<Backfill>,
    pub soft_deletes: Vec<OrphanRow>,
    /// 仍有 links/downloads、不該被刪的殘骸——需人工判斷。
    pub refused: Vec<OrphanRow>,
    /// 身分解析不出來的 work——不猜，列出來讓人處理。
    pub unresolved: Vec<WorkRow>,
}

/// 規劃 12 個可達 work 的回填。`resolve` 注入身分解析（測試時可假造）。
pub fn plan_backfill(
    works: Vec<WorkRow>,
    resolve: impl Fn(&WorkRow) -> Option<ResolvedMeta>,
) -> BackfillPlan {
    works.into_iter().fold(BackfillPlan::default(), |mut plan, w| {
        match resolve(&w) {
            Some(meta) => plan.backfills.push(Backfill {
                work_id: w.work_id,
                anime_id: w.anime_id,
                external_id: meta.external_id,
                old_title: w.title,
                new_title: meta.title,
                air_date: meta.air_date,
            }),
            None => plan.unresolved.push(w),
        }
        plan
    })
}

/// soft delete 的判定條件是「無法從任何訂閱到達」，不是「有無 anime」——
/// work 38/39 有 anime 但訂閱已被刪除，以後者為條件會漏掉。
///
/// 實測時這 39 個殘骸皆為零 links、零 downloads。若前提改變（有下載記錄），
/// 一律拒絕刪除並列入 refused，而非照刪。
pub fn plan_backfill_with_orphans(orphans: Vec<OrphanRow>) -> BackfillPlan {
    orphans.into_iter().fold(BackfillPlan::default(), |mut plan, o| {
        if o.links == 0 && o.downloads == 0 {
            plan.soft_deletes.push(o);
        } else {
            plan.refused.push(o);
        }
        plan
    })
}
```

- [ ] **Step 4: 執行測試確認通過**

Run: `cargo test -p bangumi-cli backfill 2>&1 | tail -5`
Expected: PASS（3 passed）

- [ ] **Step 5: 接上 DB 與報表**

`--dry-run`（預設）印出：

```
遷移報表 (dry-run)

BACKFILL  12 works <- 12 subscriptions
  work 16  金牌得主   -> bgm/548818  改為「金牌得主 第二季」
  work 50  てんびん     -> bgm/456080  改為「转学后班上的...」 [修正錯誤]
  ...

SOFT-DELETE 39 works（無法從任何訂閱到達的解析殘骸）
REFUSED      0 works（仍有 links/downloads）

套用? 加上 --apply
```

可達性查詢（已於產品環境驗證）：

```sql
WITH reachable AS (
  SELECT DISTINCT a.work_id
  FROM subscriptions s
  JOIN raw_anime_items r ON r.subscription_id = s.subscription_id
  JOIN anime_links l ON l.raw_item_id = r.item_id
  JOIN animes a ON a.anime_id = l.anime_id
)
SELECT w.work_id, w.title FROM anime_works w
WHERE w.work_id NOT IN (SELECT work_id FROM reachable);
```

`--apply` 時：更新 `anime_works.title` / `animes.title` / `animes.aired_date` /
`animes.season_id`、寫入 `anime_external_ids`（source `fetcher`）、
殘骸設 `is_active = false, soft_deleted_at = now()`。**不得硬刪。**

- [ ] **Step 6: 對開發資料庫驗證**

```bash
cargo run --bin bangumi-cli -- backfill-identity --dry-run
```
Expected: 報表列出 backfill 與 soft-delete，且**未寫入任何資料**（以
`SELECT count(*) FROM anime_external_ids;` 確認前後不變）。

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy -p bangumi-cli -- -D warnings
git add cli/
git commit -m "feat: add a dry-run backfill for existing wrong identities

Prod is read-only, so the operator runs this. Reachability from a
subscription — not the presence of an anime row — decides what is a parse
remnant: works 38/39 have animes but their subscription was deleted.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## 執行順序與相依

```
Task 1 (shared 型別)
  ├─> Task 2 → Task 3 → Task 4   (mikanani fetcher)
  └─> Task 5 (migration)
        ├─> Task 6 (namespace 註冊) ─┐
        ├─> Task 7 (metadata 服務) ──┤
        ├─> Task 8 (identity 服務) ──┼─> Task 10 (接上解析)
        └─> Task 9 (攝入保留 ids) ───┘        ├─> Task 11 (待認領佇列)
                                              ├─> Task 12 (封面 + resync)
                                              └─> Task 13 (回填 CLI)
```

Task 2/3/4 與 Task 5-9 之間無相依，可平行。

## 上線順序（重要）

fetcher 與 core 的 payload 變更向後相容（`#[serde(default)]`），但**語意上有順序**：

1. 先部署 core（能接收並保留 external_ids，舊 fetcher 送空陣列時走待認領佇列）
2. 再部署 metadata service（宣告 `bgm` namespace）
3. 最後部署 fetcher（開始回報身分）
4. 由維運者執行 `backfill-identity --dry-run`，核對報表後 `--apply`

若順序顛倒（先部署 fetcher），core 尚無 `metadata_namespaces` 資料，
`usable_ids` 會過濾掉全部身分，所有 item 湧入待認領佇列。
