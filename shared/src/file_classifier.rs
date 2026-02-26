use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
