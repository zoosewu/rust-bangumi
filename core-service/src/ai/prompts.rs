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

1. Analyze ALL titles and find the most common format pattern.
2. Generate ONE parser maximizing coverage.
3. Titles that don't fit go into `unmatched_titles` for a future retry.

## JSON Format

Return a **single JSON object**:

- **name** (string): Parser name, e.g. `"LoliHouse Standard Parser"`
- **condition_regex** (string): Regex that must match for this parser to activate
- **parse_regex** (string): Regex with numbered capture groups
- **priority** (number): `9999` for single-anime parsers; `50` for general-purpose
- **anime_title_source** / **anime_title_value**: `"regex"`→`"$N"` | `"static"`→fixed string
- **episode_no_source** / **episode_no_value**: `"regex"`→`"$N"` | `"static"`→`"12"` — **integer required**
- **episode_end_source** / **episode_end_value**: same as above, or `null` — integer; only for batch torrents
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

## Example

```json
{
  "name": "LoliHouse Standard Parser",
  "condition_regex": "^\\[LoliHouse\\]",
  "parse_regex": "^\\[LoliHouse\\]\\s*(.+?)\\s+-\\s*(\\d+)\\s*\\[(\\d+p)",
  "priority": 50,
  "anime_title_source": "regex", "anime_title_value": "$1",
  "episode_no_source": "regex", "episode_no_value": "$2",
  "episode_end_source": null, "episode_end_value": null,
  "series_no_source": null, "series_no_value": null,
  "subtitle_group_source": "static", "subtitle_group_value": "LoliHouse",
  "resolution_source": "regex", "resolution_value": "$3",
  "season_source": null, "season_value": null,
  "year_source": null, "year_value": null,
  "matched_titles": ["[LoliHouse] Attack on Titan - 01 [1080p HEVC]", "[LoliHouse] Attack on Titan - 02 [1080p HEVC]"],
  "unmatched_titles": ["[SomeOtherGroup] Different Format 03 (720p)"]
}
```

Return ONLY the JSON object, no extra text."#;

/// Filter 固定 Prompt 預設值
pub const DEFAULT_FIXED_FILTER_PROMPT: &str = r#"You are an anime download filter rule expert. You will receive multiple conflict groups, each containing RSS titles from different subtitle groups for the same episode. Your task is to generate ONE set of filter rules that resolves as many groups as possible.

## ⚠️ CRITICAL: JSON Escaping for Regex

In JSON strings, ALL backslashes must be doubled. This is mandatory:
- WRONG: "regex_pattern": "\[SubGroup\] Title - \d+"
- CORRECT: "regex_pattern": "\\[SubGroup\\] Title - \\d+"

## Strategy

1. Analyze ALL conflict groups and identify the most common pattern (e.g., most groups prefer a specific subtitle group).
2. Generate ONE set of filter rules targeting that pattern — prioritize resolving the **most groups**.
3. Groups that cannot be resolved by these rules go into `unresolved_groups` for a future retry.

## Filter Rule JSON Format

Return a **single JSON object** with these fields:

- **rules** (array): Filter rules, each containing:
  - **regex_pattern** (string): Regex pattern to match against titles (double-escape backslashes)
  - **is_positive** (boolean): `true` to keep matching titles; `false` to exclude matching titles
  - **rule_order** (integer): Evaluation order starting from 1 — higher value executes first
- **resolved_groups** (array of integers): 1-indexed group numbers that your rules successfully resolve
- **unresolved_groups** (array of integers): 1-indexed group numbers NOT resolved by these rules

## ⚠️ IMPORTANT: resolved_groups + unresolved_groups Must Cover Every Group

Every group index (1 through N) MUST appear in exactly one of `resolved_groups` or `unresolved_groups`. No group may be omitted.

## Rule Evaluation Logic

Rules use AND logic — a title must pass all applicable rules to be kept. Higher `rule_order` executes first.

## Example Response

Input:
```
--- Group 1 ---
1. [LoliHouse] Attack on Titan - 01 [1080p HEVC]
2. [Erai-raws] Attack on Titan - 01 [720p]

--- Group 2 ---
1. [LoliHouse] Frieren - 05 [1080p HEVC]
2. [SubParrot] Frieren - 05 [1080p]

--- Group 3 ---
1. [ANK-Raws] Solo Leveling - 03 [4K]
2. [Erai-raws] Solo Leveling - 03 [1080p]
```

```json
{
  "rules": [
    {
      "regex_pattern": "\\[LoliHouse\\]",
      "is_positive": true,
      "rule_order": 1
    }
  ],
  "resolved_groups": [1, 2],
  "unresolved_groups": [3]
}
```

Return ONLY the JSON object, no extra text."#;

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
pub fn build_parser_batch_user_prompt(titles: &[String], custom: Option<&str>) -> String {
    let list = titles
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = format!("Unmatched anime RSS titles ({} total):\n{}", titles.len(), list);
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
