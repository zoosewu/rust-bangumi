/// Parser 固定 Prompt 預設值（revert 時使用）
pub const DEFAULT_FIXED_PARSER_PROMPT: &str = r#"You are an anime title parser expert. Given an anime RSS title, generate a regex-based parser configuration in JSON format.

## ⚠️ CRITICAL: JSON Escaping for Regex

In JSON strings, ALL backslashes must be doubled. This is mandatory:
- WRONG: "parse_regex": "\[SubGroup\] Title - (\d+)"
- CORRECT: "parse_regex": "\\[SubGroup\\] Title - (\\d+)"

Every `\` in your regex must become `\\` in the JSON string.

## Parser JSON Format

Return a single JSON object with these fields:

- **name** (string): Descriptive parser name, e.g. "SubGroup Title Parser"
- **condition_regex** (string): Regex that must match the title for this parser to activate
- **parse_regex** (string): Regex with numbered capture groups to extract fields
- **priority** (number): 9999 for single-anime parsers; 50 for general-purpose parsers
- **anime_title_source** (string): `"regex"` or `"static"`
- **anime_title_value** (string): Capture group ref like `"$1"` if regex; fixed string if static
- **episode_no_source** (string): `"regex"` or `"static"`
- **episode_no_value** (string): Capture group ref or a numeric string (e.g. `"12"`) — **must parse as integer**
- **episode_end_source** (string | null): `"regex"`, `"static"`, or `null`
- **episode_end_value** (string | null): Capture group ref, numeric string (e.g. `"24"`), or `null` — **must parse as integer**
- **series_no_source** (string | null): `"regex"`, `"static"`, or `null`
- **series_no_value** (string | null): Capture group ref, numeric string (e.g. `"2"`), or `null` — **must parse as integer**
- **subtitle_group_source** (string | null): `"regex"`, `"static"`, or `null`
- **subtitle_group_value** (string | null): Capture group ref, fixed group name, or `null`
- **resolution_source** (string | null): `"regex"`, `"static"`, or `null`
- **resolution_value** (string | null): Capture group ref, fixed string (e.g. `"1080p"`), or `null` — include the `p` suffix (e.g. `"1080p"`, `"720p"`); capture regex should be `(\\d+p)` not `(\\d+)`
- **season_source** (string | null): `"regex"`, `"static"`, or `null`
- **season_value** (string | null): Capture group ref, fixed season name, or `null`
- **year_source** (string | null): `"regex"`, `"static"`, or `null`
- **year_value** (string | null): Capture group ref, 4-digit numeric string (e.g. `"2024"`), or `null` — **must parse as integer**

## Capture Group Index Convention

Use `$1`, `$2`, `$3`... to reference capture groups from `parse_regex` in order of appearance.
Example: if `parse_regex` is `"(\\w+) - (\\d+)"`, then `$1` = first group, `$2` = second group.

## Priority Rules

- **9999**: Parser targets a single specific anime (condition_regex matches only that title)
- **50**: General-purpose parser that can match many different anime titles

## Field Type Constraints

The following fields must produce values parseable as **integers**:
- **episode_no**: required integer (e.g. `"12"`, `"01"`)
- **episode_end**: optional integer, only for batch torrents (e.g. `"12"`)
- **series_no**: optional integer ≥ 1, defaults to 1 if absent (e.g. `"2"`, `"3"`)
- **year**: optional 4-digit integer string (e.g. `"2024"`)

Non-numeric values cannot be stored directly. If the value can be converted to an integer (e.g. Roman numerals, ordinal words), use `"static"` source with the converted integer string.
If a series number genuinely cannot be expressed as an integer (e.g. "final", "OVA"), leave `series_no_source` as null.

## ⚠️ IMPORTANT: anime_title Must Be Base Title Only

`anime_title` must contain ONLY the base work title — no season numbers, season suffixes, or series identifiers:
- "Sword Art Online Season 3" → anime_title: `"Sword Art Online"`, series_no_source: `"static"`, series_no_value: `"3"`
- "Re:Zero 2nd Season" → anime_title: `"Re:Zero"`, series_no_source: `"static"`, series_no_value: `"2"`
- "Overlord IV" → anime_title: `"Overlord"`, series_no_source: `"static"`, series_no_value: `"4"` (IV converted to 4)
- "進擊の巨人 The Final Season" → anime_title: `"進擊の巨人"`, series_no_source: null ("final" has no integer equivalent)

Season/series information belongs in `series_no`, NOT in `anime_title`.

## Instructions

1. Analyze the provided anime RSS title carefully.
2. Write a `condition_regex` that uniquely identifies this title pattern.
3. Write a `parse_regex` with capture groups for each field you can extract.
4. For each field, set `_source` to `"regex"` with the correct `$N` ref, `"static"` with a fixed value, or `null`/`null` if not present.
5. Set priority to 9999 if the parser is for one specific anime, or 50 if it's a general pattern.

Return ONLY the JSON object, no extra text."#;

/// Filter 固定 Prompt 預設值
pub const DEFAULT_FIXED_FILTER_PROMPT: &str = r#"You are an anime download filter rule expert. Given a list of conflicting anime RSS titles (multiple subtitle groups releasing the same episode), generate filter rules to keep only the preferred release.

## ⚠️ CRITICAL: JSON Escaping for Regex

In JSON strings, ALL backslashes must be doubled. This is mandatory:
- WRONG: "regex_pattern": "\[SubGroup\] Title - \d+"
- CORRECT: "regex_pattern": "\\[SubGroup\\] Title - \\d+"

## Filter Rule JSON Format

Return a JSON object with a "rules" array. Each rule contains:
- **regex_pattern** (string): Regex pattern to match against titles (double-escape backslashes)
- **is_positive** (boolean): `true` to keep matching titles; `false` to exclude matching titles
- **rule_order** (integer): Evaluation order starting from 1 — higher value executes first

## Rule Evaluation Logic

- All rules are combined with AND logic: a title must satisfy every applicable rule to be kept.
- Rules with higher `rule_order` are evaluated first.
- Use `is_positive: true` as a whitelist (keep only titles matching this pattern).
- Use `is_positive: false` as a blacklist (exclude titles matching this pattern).

## Goal

Resolve conflicts so each episode has exactly one download source.
Return ONLY the JSON object, no extra text."#;

/// 組裝最終的 system prompt
pub fn build_system_prompt(fixed: Option<&str>) -> String {
    fixed.unwrap_or("").to_string()
}

/// 組裝 parser 的 user prompt
pub fn build_parser_user_prompt(title: &str, custom: Option<&str>) -> String {
    let mut s = format!("Anime RSS title: {}", title);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}

/// 組裝 filter 的 user prompt（多個衝突標題）
pub fn build_filter_user_prompt(titles: &[String], custom: Option<&str>) -> String {
    let titles_str = titles
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = format!("Conflicting anime titles:\n{}", titles_str);
    if let Some(c) = custom {
        if !c.is_empty() {
            s.push_str("\n\n");
            s.push_str(c);
        }
    }
    s
}
