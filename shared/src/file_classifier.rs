use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    Video,
    Subtitle,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub path: String,
    pub file_type: FileType,
}

pub fn classify_files(files: Vec<String>) -> Vec<MediaFile> {
    files
        .into_iter()
        .map(|path| {
            let file_type = classify_extension(&path);
            MediaFile { path, file_type }
        })
        .collect()
}

fn classify_extension(path: &str) -> FileType {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "mkv" | "mp4" | "avi" | "ts" | "m2ts" | "mov" | "wmv" | "flv" | "webm" => FileType::Video,
        "ass" | "ssa" | "srt" | "vtt" | "sup" | "sub" | "idx" => FileType::Subtitle,
        _ => FileType::Other,
    }
}

pub fn extract_language_tag(path: &str) -> Option<String> {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let parts: Vec<&str> = stem.split('.').collect();
    if parts.len() >= 2 {
        Some(parts[parts.len() - 1].to_string())
    } else {
        None
    }
}

pub fn collect_files_recursive(path: &Path) -> Vec<String> {
    let mut result = Vec::new();
    collect_files_inner(path, &mut result);
    result
}

fn collect_files_inner(path: &Path, result: &mut Vec<String>) {
    if path.is_file() {
        match path.to_str() {
            Some(s) => result.push(s.to_string()),
            None => tracing::warn!("Skipping non-UTF-8 path: {:?}", path),
        }
        return;
    }
    if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_files_inner(&entry.path(), result);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LanguageCodeMap(HashMap<String, String>);

impl LanguageCodeMap {
    pub fn from_entries(entries: Vec<(String, String)>) -> Self {
        let map = entries
            .into_iter()
            .map(|(k, v)| (k.to_uppercase(), v))
            .collect();
        LanguageCodeMap(map)
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let raw: HashMap<String, String> = serde_json::from_str(&content)?;
        let map = raw
            .into_iter()
            .map(|(k, v)| (k.to_uppercase(), v))
            .collect();
        Ok(LanguageCodeMap(map))
    }

    pub fn normalize(&self, tag: &str) -> String {
        let key = tag.to_uppercase();
        self.0.get(&key).cloned().unwrap_or_else(|| tag.to_string())
    }
}

// ============ Episode Extraction — Chain of Responsibility ============

/// Trait for episode number extraction strategies.
/// Returns `Some(n)` if exactly one candidate is in `expected`; `None` to pass to next handler.
pub trait EpisodeExtractHandler: Send + Sync {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32>;
}

fn unique_match(candidates: Vec<i32>, expected: &HashSet<i32>) -> Option<i32> {
    let matches: Vec<i32> = candidates
        .into_iter()
        .filter(|n| expected.contains(n))
        .collect();
    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

/// Handler 1: explicit markers — EP01, E01 (standalone), 第01話
///
/// Standalone `E` is only matched when NOT preceded by an alphanumeric character
/// (prevents false match on S02E07 season-episode notation).
pub struct ExplicitMarkerHandler;

impl EpisodeExtractHandler for ExplicitMarkerHandler {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32> {
        // EP01 — "EP" prefix (unambiguous)
        let re_ep = Regex::new(r"(?i)EP(\d{1,3})").unwrap();
        // 第01話 — CJK episode marker
        let re_cjk = Regex::new(r"第(\d{1,3})").unwrap();
        // E01 standalone — E must NOT be preceded by alphanumeric (rules out S02E07)
        let re_e = Regex::new(r"(?i)(?:[^A-Za-z0-9]|^)E(\d{1,3})").unwrap();

        let mut candidates: Vec<i32> = Vec::new();
        for c in re_ep.captures_iter(stem) {
            if let Ok(n) = c[1].parse::<i32>() { candidates.push(n); }
        }
        for c in re_cjk.captures_iter(stem) {
            if let Ok(n) = c[1].parse::<i32>() { candidates.push(n); }
        }
        for c in re_e.captures_iter(stem) {
            if let Ok(n) = c[1].parse::<i32>() { candidates.push(n); }
        }
        unique_match(candidates, expected)
    }
}

/// Handler 2: separator-bounded numbers — "- 07 ", "_07_", "-07v2"
pub struct DashSeparatorHandler;

impl EpisodeExtractHandler for DashSeparatorHandler {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32> {
        let re = Regex::new(r"(?:[\s\-_\.])(\d{1,3})(?:[\s\-_\.v]|$)").unwrap();
        let candidates = re
            .captures_iter(stem)
            .filter_map(|c| c[1].parse::<i32>().ok())
            .collect();
        unique_match(candidates, expected)
    }
}

/// Handler 3: any 1–3 digit sequence at a word boundary.
///
/// Uses `\b` (word boundary) instead of lookahead/lookbehind since the `regex`
/// crate does not support look-around assertions.
pub struct IsolatedDigitHandler;

impl EpisodeExtractHandler for IsolatedDigitHandler {
    fn extract(&self, stem: &str, expected: &HashSet<i32>) -> Option<i32> {
        let re = Regex::new(r"\b(\d{1,3})\b").unwrap();
        let candidates = re
            .captures_iter(stem)
            .filter_map(|c| c[1].parse::<i32>().ok())
            .collect();
        unique_match(candidates, expected)
    }
}

/// Build the default three-handler chain (ExplicitMarker → DashSeparator → IsolatedDigit).
pub fn build_default_chain() -> Vec<Box<dyn EpisodeExtractHandler>> {
    vec![
        Box::new(ExplicitMarkerHandler),
        Box::new(DashSeparatorHandler),
        Box::new(IsolatedDigitHandler),
    ]
}

/// Walk the chain until a handler returns Some; otherwise return None.
pub fn extract_episode_from_stem(
    stem: &str,
    expected: &HashSet<i32>,
    chain: &[Box<dyn EpisodeExtractHandler>],
) -> Option<i32> {
    chain.iter().find_map(|h| h.extract(stem, expected))
}

/// Match all files in a completed batch torrent to their episode numbers.
///
/// Returns `episode_no → (Option<video_path>, Vec<subtitle_paths>)`.
/// Episodes that cannot be uniquely matched are absent from the map.
pub fn match_batch_files(
    files: &[String],
    episode_nos: &[i32],
    chain: &[Box<dyn EpisodeExtractHandler>],
) -> HashMap<i32, (Option<String>, Vec<String>)> {
    let expected: HashSet<i32> = episode_nos.iter().copied().collect();
    let classified = classify_files(files.to_vec());
    let mut result: HashMap<i32, (Option<String>, Vec<String>)> = HashMap::new();

    for mf in classified.iter().filter(|f| f.file_type == FileType::Video) {
        let stem = Path::new(&mf.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if let Some(ep) = extract_episode_from_stem(stem, &expected, chain) {
            result.entry(ep).or_default().0 = Some(mf.path.clone());
        }
    }

    for mf in classified.iter().filter(|f| f.file_type == FileType::Subtitle) {
        let stem = Path::new(&mf.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if let Some(ep) = extract_episode_from_stem(stem, &expected, chain) {
            result.entry(ep).or_default().1.push(mf.path.clone());
        }
    }

    result
}

#[cfg(test)]
mod episode_tests {
    use super::*;
    use std::collections::HashSet;

    fn expected(range: std::ops::RangeInclusive<i32>) -> HashSet<i32> {
        range.collect()
    }

    fn chain() -> Vec<Box<dyn EpisodeExtractHandler>> {
        build_default_chain()
    }

    // ExplicitMarkerHandler tests
    #[test]
    fn test_explicit_ep_prefix() {
        let ep = extract_episode_from_stem("Show EP07 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    #[test]
    fn test_explicit_e_prefix_uppercase() {
        let ep = extract_episode_from_stem("Show E07 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    #[test]
    fn test_explicit_cjk_marker() {
        let ep = extract_episode_from_stem("Show 第07話 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    // DashSeparatorHandler tests
    #[test]
    fn test_dash_separator() {
        let ep = extract_episode_from_stem("[Group] Show - 07 [1080p][ABCD]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    #[test]
    fn test_dash_separator_version_suffix() {
        // "07v2" — the v2 should not block matching
        let ep = extract_episode_from_stem("[Group] Show - 07v2 [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    // IsolatedDigitHandler tests
    #[test]
    fn test_isolated_digit_fallback() {
        let ep = extract_episode_from_stem("show.07.mkv.stem", &expected(1..=12), &chain());
        assert_eq!(ep, Some(7));
    }

    // Ambiguity → None
    #[test]
    fn test_ambiguous_returns_none() {
        // "02" and "07" both in expected range — ambiguous
        let ep = extract_episode_from_stem("S02E07", &expected(1..=12), &chain());
        assert_eq!(ep, None);
    }

    // Out of range → None
    #[test]
    fn test_out_of_range_ignored() {
        // Only number is 1080 which is not in expected range
        let ep = extract_episode_from_stem("show [1080p]", &expected(1..=12), &chain());
        assert_eq!(ep, None);
    }

    // Zero-padded parsing
    #[test]
    fn test_zero_padded_parsed_correctly() {
        let ep = extract_episode_from_stem("[Group] Show - 01 [720p]", &expected(1..=12), &chain());
        assert_eq!(ep, Some(1));
    }

    // match_batch_files tests
    #[test]
    fn test_match_batch_files_video_and_subtitle() {
        let files = vec![
            "/dl/Show/[G] Show - 01 [1080p].mkv".to_string(),
            "/dl/Show/[G] Show - 01 [1080p].zh.ass".to_string(),
            "/dl/Show/[G] Show - 02 [1080p].mkv".to_string(),
            "/dl/Show/[G] Show - 02 [1080p].zh.ass".to_string(),
        ];
        let chain = build_default_chain();
        let result = match_batch_files(&files, &[1, 2], &chain);

        assert_eq!(result.get(&1).unwrap().0.as_deref(), Some("/dl/Show/[G] Show - 01 [1080p].mkv"));
        assert_eq!(result.get(&1).unwrap().1, vec!["/dl/Show/[G] Show - 01 [1080p].zh.ass"]);
        assert_eq!(result.get(&2).unwrap().0.as_deref(), Some("/dl/Show/[G] Show - 02 [1080p].mkv"));
    }

    #[test]
    fn test_match_batch_files_unmatched_episode_absent() {
        // ep 3 has no corresponding file
        let files = vec![
            "/dl/Show - 01.mkv".to_string(),
            "/dl/Show - 02.mkv".to_string(),
        ];
        let chain = build_default_chain();
        let result = match_batch_files(&files, &[1, 2, 3], &chain);

        assert!(result.contains_key(&1));
        assert!(result.contains_key(&2));
        assert!(!result.contains_key(&3));
    }

    #[test]
    fn test_match_batch_files_single_episode_not_confused() {
        // Only one expected episode → no ambiguity
        let files = vec![
            "/dl/Show - 05 [1080p].mkv".to_string(),
        ];
        let chain = build_default_chain();
        let result = match_batch_files(&files, &[5], &chain);
        assert_eq!(result.get(&5).unwrap().0.as_deref(), Some("/dl/Show - 05 [1080p].mkv"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_video_extensions() {
        let files = vec![
            "a.mkv".to_string(),
            "b.mp4".to_string(),
            "c.avi".to_string(),
            "d.ts".to_string(),
        ];
        let result = classify_files(files);
        for mf in &result {
            assert_eq!(mf.file_type, FileType::Video, "expected Video for {}", mf.path);
        }
    }

    #[test]
    fn test_classify_subtitle_extensions() {
        let files = vec![
            "a.ass".to_string(),
            "b.ssa".to_string(),
            "c.srt".to_string(),
            "d.vtt".to_string(),
        ];
        let result = classify_files(files);
        for mf in &result {
            assert_eq!(mf.file_type, FileType::Subtitle, "expected Subtitle for {}", mf.path);
        }
    }

    #[test]
    fn test_classify_other_extensions() {
        let files = vec!["image.jpg".to_string(), "readme.txt".to_string()];
        let result = classify_files(files);
        for mf in &result {
            assert_eq!(mf.file_type, FileType::Other, "expected Other for {}", mf.path);
        }
    }

    #[test]
    fn test_classify_mixed_files() {
        let files = vec![
            "video.mkv".to_string(),
            "sub.ass".to_string(),
            "image.png".to_string(),
        ];
        let result = classify_files(files);
        assert_eq!(result[0].file_type, FileType::Video);
        assert_eq!(result[1].file_type, FileType::Subtitle);
        assert_eq!(result[2].file_type, FileType::Other);
    }

    #[test]
    fn test_language_code_map_normalize_known() {
        let map = LanguageCodeMap::from_entries(vec![
            ("TC".to_string(), "zh-TW".to_string()),
            ("SC".to_string(), "zh-CN".to_string()),
        ]);
        assert_eq!(map.normalize("TC"), "zh-TW");
        assert_eq!(map.normalize("SC"), "zh-CN");
    }

    #[test]
    fn test_language_code_map_normalize_unknown() {
        let map = LanguageCodeMap::from_entries(vec![]);
        assert_eq!(map.normalize("XX"), "XX");
    }

    #[test]
    fn test_language_code_map_case_insensitive() {
        let map = LanguageCodeMap::from_entries(vec![
            ("TC".to_string(), "zh-TW".to_string()),
        ]);
        assert_eq!(map.normalize("tc"), "zh-TW");
    }

    #[test]
    fn test_extract_language_tag_dotted_stem() {
        let result = extract_language_tag("/downloads/sub.TC.ass");
        assert_eq!(result, Some("TC".to_string()));
    }

    #[test]
    fn test_extract_language_tag_simple_stem() {
        let result = extract_language_tag("/downloads/subtitle.ass");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_language_tag_nested() {
        let result = extract_language_tag("/downloads/subtitle.CHS.srt");
        assert_eq!(result, Some("CHS".to_string()));
    }
}
