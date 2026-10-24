/// Parser 結構化輸出 JSON Schema
pub fn parser_schema() -> serde_json::Value {
    use serde_json::json;

    let nullable_source = json!({
        "anyOf": [
            {"type": "string", "enum": ["regex", "static"]},
            {"type": "null"}
        ]
    });
    let nullable_string = json!({"anyOf": [{"type": "string"}, {"type": "null"}]});

    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "name", "condition_regex", "parse_regex", "priority",
            "anime_title_source", "anime_title_value",
            "episode_no_source", "episode_no_value",
            "episode_end_source", "episode_end_value",
            "series_no_source", "series_no_value",
            "subtitle_group_source", "subtitle_group_value",
            "resolution_source", "resolution_value",
            "season_source", "season_value",
            "year_source", "year_value",
            "matched_titles", "unmatched_titles"
        ],
        "properties": {
            "name": {"type": "string"},
            "condition_regex": {"type": "string"},
            "parse_regex": {"type": "string"},
            "priority": {"type": "integer"},
            "anime_title_source": {"type": "string", "enum": ["regex", "static"]},
            "anime_title_value": {"type": "string"},
            "episode_no_source": {"type": "string", "enum": ["regex", "static"]},
            "episode_no_value": {"type": "string"},
            "episode_end_source": nullable_source.clone(),
            "episode_end_value": nullable_string.clone(),
            "series_no_source": nullable_source.clone(),
            "series_no_value": nullable_string.clone(),
            "subtitle_group_source": nullable_source.clone(),
            "subtitle_group_value": nullable_string.clone(),
            "resolution_source": nullable_source.clone(),
            "resolution_value": nullable_string.clone(),
            "season_source": nullable_source.clone(),
            "season_value": nullable_string.clone(),
            "year_source": nullable_source,
            "year_value": nullable_string,
            "matched_titles": {"type": "array", "items": {"type": "string"}},
            "unmatched_titles": {"type": "array", "items": {"type": "string"}}
        }
    })
}

/// Filter 結構化輸出 JSON Schema
pub fn filter_schema() -> serde_json::Value {
    use serde_json::json;
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["rules", "resolved_groups", "unresolved_groups"],
        "properties": {
            "rules": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["regex_pattern", "is_positive", "rule_order"],
                    "properties": {
                        "regex_pattern": {"type": "string"},
                        "is_positive": {"type": "boolean"},
                        "rule_order": {"type": "integer"}
                    }
                }
            },
            "resolved_groups": {"type": "array", "items": {"type": "integer"}},
            "unresolved_groups": {"type": "array", "items": {"type": "integer"}}
        }
    })
}

/// AI 要產生的東西：解析器或過濾規則（各自有一份固定 prompt）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    Parser,
    Filter,
}

/// 一次請求的固定 prompt（三層：全域 → 模型 → 客製化，客製化另外附在 user prompt）。
#[derive(Debug, Clone)]
pub struct FixedPrompt {
    pub kind: PromptKind,
    /// 全域的固定 prompt（或這次指定的）
    pub text: String,
    /// true：這次指定的（例如重新生成時編輯過），每個 provider 都用它；
    /// false：provider 有自己的模型 prompt 時用模型的
    pub pinned: bool,
}

impl FixedPrompt {
    /// 某個 provider 實際要用的固定 prompt
    pub fn for_provider<'a>(&'a self, model_prompt: Option<&'a str>) -> &'a str {
        match model_prompt.filter(|p| !p.trim().is_empty()) {
            Some(p) if !self.pinned => p,
            _ => &self.text,
        }
    }
}

/// Parser 固定 Prompt 預設值（revert 時使用）
pub const DEFAULT_FIXED_PARSER_PROMPT: &str = r#"You are an anime title parser expert. Generate ONE regex-based parser that covers as many of the provided unmatched RSS titles as possible.

## ⚠️ Regex JSON Escaping — Square Brackets

RSS titles contain literal `[` and `]` (e.g. `[SubGroup]`, `[07]`, `[1080p]`). To match them in regex:
- `[` → write `\\[` in JSON
- `]` → write `\\]` in JSON

**WRONG** — using `$` as a bracket substitute (common mistake):
- `"^$SubGroup$"` — `$` is NOT `[` or `]`
- `"$$(\\d+)$$"` — this does NOT match `[07]`
- `"\\$"` — `\$` is an invalid JSON escape and will cause a parse error

**CORRECT:**
- `"^\\[SubGroup\\]"` — matches `[SubGroup]` at start
- `"\\[(\\d+)\\]"` — matches `[07]`, captures `07`

All backslashes must be doubled in JSON: `\[` in regex → `\\[` in JSON string.

## Strategy

1. Identify the **exact anime title** from the input titles.
2. Detect if the title is a **batch/collection release** (e.g. `[01-13 合集]`, `Batch`, `Fin`) — if so, extract the episode range into `episode_no` + `episode_end` (see Batch Torrents section).
3. Generate ONE parser that matches **only that specific anime** — lock every fixed structural element.
4. Titles with an incompatible format go into `unmatched_titles` for a future retry.

## JSON Format

Return a **single JSON object**:

- **name** (string): Parser name including group + title, e.g. `"LoliHouse - Attack on Titan"`
- **condition_regex** (string): MUST contain the **literal anime title** (regex-escaped) in addition to the subtitle group. Lock subtitle group + title + separator style. e.g. `^\\[LoliHouse\\]\\s*Attack on Titan\\s*-\\s*\\d+`
- **parse_regex** (string): MUST contain the **literal anime title**. Lock every fixed structural element (group brackets, separator, episode position, resolution format). Use capture groups only for variable parts. e.g. `^\\[LoliHouse\\]\\s*Attack on Titan\\s*-\\s*(\\d+)\\s*\\[(\\d+[Pp])`
- **priority** (number): Always `9999`
- **anime_title_source** / **anime_title_value**: Always `"static"` / exact base title. NEVER use `"regex"` to extract the title dynamically.
- **episode_no_source** / **episode_no_value**: `"regex"`→`"$N"` | `"static"`→`"12"` — **integer required**
- **episode_end_source** / **episode_end_value**: same as above, or `null` — integer; for batch/collection torrents containing multiple episodes (see Batch Torrents section below)
- **series_no_source** / **series_no_value**: same, or `null` — integer ≥ 1
- **subtitle_group_source** / **subtitle_group_value**: `"regex"`→`"$N"` | `"static"`→name | `null`
- **resolution_source** / **resolution_value**: same, or `null` — value includes `p` suffix (e.g. `"1080p"`); use `(\\d+[Pp])` in parse_regex to match both `1080p` and `1080P`
- **season_source** / **season_value**: `"regex"`→`"$N"` | `"static"`→name | `null`
- **year_source** / **year_value**: same, or `null` — 4-digit integer required
- **matched_titles** (string[]): Input titles matched by this parser
- **unmatched_titles** (string[]): All remaining input titles — every input title must appear in exactly one list

Capture groups in `parse_regex` are referenced as `$1`, `$2`, ... in `_value` fields **only**. Do NOT use `$` to represent brackets `[` `]` in regex patterns — use `\\[` and `\\]` instead.

Non-numeric series values (e.g. Roman numerals) must be converted to integers via `"static"` source. If conversion is impossible (e.g. "final", "OVA"), set `series_no_source` to `null`.

## ⚠️ CJK Characters and Case Sensitivity

**`\\d+` does NOT match CJK numerals.** Chinese/Japanese season markers like `第三季`, `第二期` use CJK characters — `三`, `二`, `四` are NOT digits. If a season/number appears as CJK text, use `series_no_source: "static"` with the Arabic integer and match the CJK text literally in parse_regex (e.g. `第三季` as a literal string, not a capture group).

**Regex is case-sensitive.** `(\\d+p)` does NOT match `1080P`. Always use `(\\d+[Pp])` for resolution.

## anime_title: Base Title Only

Strip season numbers and identifiers — `anime_title` must be the base work title only:
- "Re:Zero 2nd Season" → `"Re:Zero"`, series_no `"static"`/`"2"`
- "Overlord IV" → `"Overlord"`, series_no `"static"`/`"4"` (Roman → integer)
- "進擊の巨人 The Final Season" → `"進擊の巨人"`, series_no `null`

## Batch Torrents (合輯)

Some titles are batch/collection releases containing multiple episodes in a single torrent. Recognize these patterns:
- `[01-13 合集]`, `[01-13]`, `[01~13]` — episode range in brackets
- `Batch`, `Complete`, `Fin` — batch/final markers
- `合集`, `全集` — Chinese batch markers

For batch titles, set **both** `episode_no` (start) and `episode_end` (end):
- `episode_no_source`/`episode_no_value`: the **first** episode number
- `episode_end_source`/`episode_end_value`: the **last** episode number

For single-episode titles, set `episode_end_source` and `episode_end_value` to `null`.

### Batch Example

Title: `[LoliHouse] Jigokuraku [01-13 合集][WebRip 1080p HEVC-10bit AAC][Fin]`

```json
{
  "name": "LoliHouse - Jigokuraku (Batch)",
  "condition_regex": "^\\[LoliHouse\\]\\s*Jigokuraku\\s*\\[\\d+-\\d+",
  "parse_regex": "^\\[LoliHouse\\]\\s*Jigokuraku\\s*\\[(\\d+)-(\\d+)\\s*合集\\]\\[.*?(\\d+[Pp])",
  "priority": 9999,
  "anime_title_source": "static", "anime_title_value": "Jigokuraku",
  "episode_no_source": "regex", "episode_no_value": "$1",
  "episode_end_source": "regex", "episode_end_value": "$2",
  "series_no_source": null, "series_no_value": null,
  "subtitle_group_source": "static", "subtitle_group_value": "LoliHouse",
  "resolution_source": "regex", "resolution_value": "$3",
  "season_source": null, "season_value": null,
  "year_source": null, "year_value": null,
  "matched_titles": ["[LoliHouse] Jigokuraku [01-13 合集][WebRip 1080p HEVC-10bit AAC][Fin]"],
  "unmatched_titles": []
}
```

## Single-Episode Example

```json
{
  "name": "LoliHouse - Attack on Titan",
  "condition_regex": "^\\[LoliHouse\\]\\s*Attack on Titan\\s*-\\s*\\d+",
  "parse_regex": "^\\[LoliHouse\\]\\s*Attack on Titan\\s*-\\s*(\\d+)\\s*\\[(\\d+[Pp])",
  "priority": 9999,
  "anime_title_source": "static", "anime_title_value": "Attack on Titan",
  "episode_no_source": "regex", "episode_no_value": "$1",
  "episode_end_source": null, "episode_end_value": null,
  "series_no_source": null, "series_no_value": null,
  "subtitle_group_source": "static", "subtitle_group_value": "LoliHouse",
  "resolution_source": "regex", "resolution_value": "$2",
  "season_source": null, "season_value": null,
  "year_source": null, "year_value": null,
  "matched_titles": ["[LoliHouse] Attack on Titan - 01 [1080p HEVC]", "[LoliHouse] Attack on Titan - 02 [1080p HEVC]"],
  "unmatched_titles": ["[SomeOtherGroup] Different Format 03 (720p)"]
}
```

Return ONLY the JSON object, no extra text."#;

/// Filter 固定 Prompt 預設值
pub const DEFAULT_FIXED_FILTER_PROMPT: &str = r#"You write filter rules that keep ONE version per episode. Each conflict group lists the same episode released in different versions. Choose the version by the user's preferences given after the groups.

How rules combine — read carefully:
- A title is kept only if it matches ALL positive rules (is_positive true) and NONE of the negative rules.
- So write exactly ONE positive rule that matches only the preferred version, e.g. its language tag. Never write one positive rule per version: several positive rules must all match at once, so nothing would be kept.
- Add negative rules only when one positive rule cannot exclude the rest.

Each regex_pattern contains the subtitle group and the anime title plus the part that singles out the preferred version — never a group name alone. Escape only [ and ]; other characters such as 【】★ are written as-is. The output is JSON, so double every backslash: "\\[", "\\d+". rule_order starts at 1. resolved_groups / unresolved_groups hold 1-based group numbers; every group appears in exactly one of them.

Example — groups of "[Group] Frieren [05][1080p][繁日内嵌]" and "[Group] Frieren [05][1080p][简日内嵌]", user prefers traditional Chinese:
{"rules": [{"regex_pattern": "^\\[Group\\]\\s*Frieren\\s*\\[\\d+\\].*\\[繁日内嵌\\]", "is_positive": true, "rule_order": 1}], "resolved_groups": [1], "unresolved_groups": []}"#;

/// 將空字串或純空白視為 None，用於 prompt cascade fallback
pub fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty())
}

/// 組裝最終的 system prompt
pub fn build_system_prompt(fixed: Option<&str>) -> String {
    fixed.unwrap_or("").to_string()
}

/// 組裝 parser 的 user prompt（單一標題，用於 regenerate 端點）
pub fn build_parser_user_prompt(title: &str, custom: Option<&str>) -> String {
    let mut s = format!("Unmatched anime RSS titles (1 total):\n1. {}", title);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

/// 組裝 parser 的 batch user prompt（多個標題，用於批次生成）
/// 一次送給 AI 的集數。同一部作品各集標題的格式相同，前幾集就足以歸納出規則；
/// 送更多只會多耗 token（Groq 免費方案每分鐘 8K token），規則確認後會套用到其餘集數。
pub const AI_SAMPLE_EPISODES: usize = 3;

/// 依集數排好的項目中，取送給 AI 的前幾集
pub fn first_episodes<T>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    items.into_iter().take(AI_SAMPLE_EPISODES).collect()
}

/// 從還沒解析的標題粗略猜出集數（`[05]`、` - 05 `、`第05話`、`[01-12]`），
/// 只用來把同一集的不同版本放在一起，不是解析。猜不出來時回傳 None。
pub fn guess_episode_key(title: &str) -> Option<String> {
    static EPISODE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let episode = EPISODE.get_or_init(|| {
        regex::Regex::new(
            r"(?:\[|【|第|\s-\s*|\sEP?)(\d{1,3}(?:\s*[-~]\s*\d{1,3})?)(?:v\d+)?(?:\]|】|話|话|集|\s|\(|\[|$)",
        )
        .expect("valid episode regex")
    });
    episode.captures(title).map(|c| {
        c[1].split_whitespace()
            .collect::<String>()
            .replace('~', "-")
    })
}

/// 依發布順序排好的標題中，取前幾集的**所有版本**（一集有三個版本就會有三個標題）。
/// 猜不出集數的標題各自算一集。
pub fn titles_of_first_episodes(titles: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut episodes: Vec<String> = Vec::new();
    titles
        .into_iter()
        .enumerate()
        .filter(|(i, title)| {
            let key = guess_episode_key(title).unwrap_or_else(|| format!("#{i}"));
            if episodes.contains(&key) {
                true
            } else if episodes.len() < AI_SAMPLE_EPISODES {
                episodes.push(key);
                true
            } else {
                false
            }
        })
        .map(|(_, title)| title)
        .collect()
}

pub fn build_parser_batch_user_prompt(titles: &[String], custom: Option<&str>) -> String {
    let list = titles
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = format!(
        "Unmatched anime RSS titles ({} total):\n{}",
        titles.len(),
        list
    );
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

/// 組裝 filter 的 user prompt（單一衝突群組，多個衝突標題）
pub fn build_filter_user_prompt(titles: &[String], custom: Option<&str>) -> String {
    let titles_str = titles
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = format!(
        "Conflict groups (1 total):\n\n--- Group 1 ---\n{}",
        titles_str
    );
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

/// 組裝 filter 的 batch user prompt（多個衝突群組）
pub fn build_filter_batch_user_prompt(groups: &[Vec<String>], custom: Option<&str>) -> String {
    let groups_str = groups
        .iter()
        .enumerate()
        .map(|(i, titles)| {
            let title_list = titles
                .iter()
                .enumerate()
                .map(|(j, t)| format!("{}. {}", j + 1, t))
                .collect::<Vec<_>>()
                .join("\n");
            format!("--- Group {} ---\n{}", i + 1, title_list)
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut s = format!(
        "Conflict groups ({} total):\n\n{}",
        groups.len(),
        groups_str
    );
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

#[cfg(test)]
mod sample_tests {
    use super::*;

    /// 預設 prompt 裡的 JSON 範例
    fn json_examples(prompt: &str) -> Vec<serde_json::Value> {
        prompt
            .split("```json")
            .skip(1)
            .filter_map(|block| block.split("```").next())
            .chain(prompt.lines().filter(|l| l.starts_with("{\"rules\"")))
            .map(|json| serde_json::from_str(json).expect("example is valid JSON"))
            .collect()
    }

    #[test]
    fn parser_examples_refer_only_to_captures_that_exist() {
        let examples = json_examples(DEFAULT_FIXED_PARSER_PROMPT);
        assert_eq!(examples.len(), 2);
        for example in examples {
            let captures = regex::Regex::new(example["parse_regex"].as_str().unwrap())
                .unwrap()
                .captures_len()
                - 1;
            let refs = example
                .as_object()
                .unwrap()
                .iter()
                .filter(|(k, _)| k.ends_with("_value"))
                .filter_map(|(_, v)| v.as_str()?.strip_prefix('$')?.parse::<usize>().ok());
            for n in refs {
                assert!(
                    n <= captures,
                    "{} uses ${n} of {captures} captures",
                    example["name"]
                );
            }
        }
    }

    #[test]
    fn the_filter_example_keeps_only_the_preferred_version() {
        let example = &json_examples(DEFAULT_FIXED_FILTER_PROMPT)[0];
        let rule =
            regex::Regex::new(example["rules"][0]["regex_pattern"].as_str().unwrap()).unwrap();
        assert!(rule.is_match("[Group] Frieren [05][1080p][繁日内嵌]"));
        assert!(!rule.is_match("[Group] Frieren [05][1080p][简日内嵌]"));
    }

    #[test]
    fn only_the_first_episodes_are_sent() {
        assert_eq!(first_episodes(1..=26), vec![1, 2, 3]);
        assert_eq!(first_episodes(vec!["only"]), vec!["only"]);
    }

    #[test]
    fn episode_numbers_are_guessed_from_common_title_styles() {
        let cases = [
            (
                "[LoliHouse] Super no Ura - 05 [WebRip 1080p HEVC-10bit AAC][简繁内封字幕]",
                Some("05"),
            ),
            (
                "【喵萌奶茶屋】★04月新番★[欺诈游戏 / LIAR GAME][01][1080p][繁日双语]",
                Some("01"),
            ),
            (
                "[绿茶字幕组] 青之箱 第二季 / Ao no Hako S2 [01][WebRip][1080p][繁日内嵌]",
                Some("01"),
            ),
            (
                "【豌豆字幕组】[药屋少女的呢喃 S3][01(49)][繁体][1080P][MP4]",
                Some("01"),
            ),
            (
                "[绿茶字幕组] 金牌得主 第二季 / Medalist S2 [14-22][WebRip][1080p]",
                Some("14-22"),
            ),
            ("[字幕组] 某动画 第12話 [1080p]", Some("12")),
            (
                "[黒ネズミたち] 罗梅莉亚战记 - 01 (CR 1920x1080 AVC AAC MKV)",
                Some("01"),
            ),
            (
                "[樱桃花字幕组]尼古喵喵 Yanineko -2[1080p][简日内嵌]",
                Some("2"),
            ),
            (
                "[樱桃花字幕组]尼古喵喵 Yanineko  - 1[1080p][简日内嵌]",
                Some("1"),
            ),
            ("Some Movie [1080p][BDRip]", None),
        ];
        for (title, expected) in cases {
            assert_eq!(guess_episode_key(title).as_deref(), expected, "{title}");
        }
    }

    #[test]
    fn every_version_of_the_first_three_episodes_is_sent() {
        let titles = [
            "[G] T [01][简日]",
            "[G] T [01][繁日]",
            "[G] T [01][简繁日内封]",
            "[G] T [02][简日]",
            "[G] T [02][繁日]",
            "[G] T [03][繁日]",
            "[G] T [04][繁日]",
            "[G] T [02][简繁日内封]",
        ]
        .map(String::from);
        let sent = titles_of_first_episodes(titles);
        assert_eq!(sent.len(), 7);
        assert!(sent.iter().all(|t| !t.contains("[04]")));
        assert!(sent.contains(&"[G] T [02][简繁日内封]".to_string()));
    }

    #[test]
    fn titles_without_an_episode_number_count_as_their_own_episode() {
        let titles = [
            "Movie A [1080p]",
            "Movie B [1080p]",
            "Movie C [1080p]",
            "Movie D [1080p]",
        ]
        .map(String::from);
        assert_eq!(titles_of_first_episodes(titles).len(), 3);
    }
}
