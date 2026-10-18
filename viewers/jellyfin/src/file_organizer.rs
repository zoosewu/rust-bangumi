use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;

#[allow(dead_code)]
static EPISODE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)s(\d+)e(\d+)|\[(\d+)\]").unwrap());

/// `Path::exists` without blocking the async worker: a stat on a busy external disk
/// can take seconds, and the viewer runs on a single worker thread.
/// 我們寫進媒體庫、可以隨資料夾一起清掉的檔案
const GENERATED_FILES: [&str; 3] = ["tvshow.nfo", "season.nfo", "poster.jpg"];

/// 同一檔案系統用 rename，否則複製後刪除
async fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if fs::rename(from, to).await.is_err() {
        fs::copy(from, to).await?;
        let _ = fs::remove_file(from).await;
    }
    Ok(())
}

/// 資料夾裡只有 `GENERATED_FILES` 時刪掉它們和資料夾，回傳是否已刪除。
async fn remove_if_only_generated(dir: &Path) -> bool {
    let Ok(mut entries) = fs::read_dir(dir).await else {
        return false;
    };
    let mut generated = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !GENERATED_FILES.contains(&name.as_str()) {
            return false;
        }
        generated.push(entry.path());
    }
    for file in &generated {
        let _ = fs::remove_file(file).await;
    }
    let removed = fs::remove_dir(dir).await.is_ok();
    if removed {
        tracing::info!("Removed directory: {}", dir.display());
    }
    removed
}

async fn exists(path: &Path) -> bool {
    fs::try_exists(path).await.unwrap_or(false)
}

#[derive(Clone, Debug)]
pub struct FileOrganizer {
    source_dir: PathBuf,
    library_dir: PathBuf,
    language_codes: shared::LanguageCodeMap,
}

impl FileOrganizer {
    pub fn new(
        source_dir: PathBuf,
        library_dir: PathBuf,
        language_codes: shared::LanguageCodeMap,
    ) -> Self {
        Self {
            source_dir,
            library_dir,
            language_codes,
        }
    }

    /// 解析下載檔案路徑：將容器內部路徑（/downloads/...）對應到本地 source_dir
    ///
    /// - 正式環境：viewer 容器內 /downloads 已掛載，路徑直接可用
    /// - 開發環境：viewer 以 cargo run 執行，/downloads 不存在，
    ///   需對應到 DOWNLOADS_DIR（如 ./tmp/bangumi-downloads）
    pub async fn resolve_download_path(&self, file_path: &str) -> PathBuf {
        let path = Path::new(file_path);
        if exists(path).await {
            return path.to_path_buf();
        }
        // 嘗試將 /downloads/... 前綴替換為 source_dir
        if let Ok(relative) = path.strip_prefix("/downloads") {
            let mapped = self.source_dir.join(relative);
            if exists(&mapped).await {
                return mapped;
            }
        }
        // Fallback：原始路徑（organize_episode 會回報 file not found）
        path.to_path_buf()
    }

    pub async fn organize_episode(
        &self,
        anime_title: &str,
        season: u32,
        episode: u32,
        source_file: &Path,
    ) -> anyhow::Result<PathBuf> {
        let season_dir = self
            .library_dir
            .join(Self::sanitize_filename(anime_title))
            .join(format!("Season {:02}", season));

        let extension = source_file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");

        let new_filename = format!(
            "{} - S{:02}E{:02}.{}",
            Self::sanitize_filename(anime_title),
            season,
            episode,
            extension
        );

        let target_path = season_dir.join(new_filename);

        if !exists(source_file).await {
            // A repeated sync of an episode already moved into the library: report the
            // file that is there instead of failing (a failure would undo the first sync)
            if exists(&target_path).await {
                tracing::info!(
                    "Already organized: {} (source {} is gone)",
                    target_path.display(),
                    source_file.display()
                );
                return Ok(target_path);
            }
            return Err(anyhow::anyhow!(
                "Source file does not exist: {}",
                source_file.display()
            ));
        }

        fs::create_dir_all(&season_dir).await?;

        move_file(source_file, &target_path).await?;

        tracing::info!(
            "Organized: {} -> {}",
            source_file.display(),
            target_path.display()
        );
        Ok(target_path)
    }

    /// Move an already-organized episode to a new location based on updated metadata.
    /// Returns the new target path.
    pub async fn move_episode(
        &self,
        current_path: &Path,
        new_anime_title: &str,
        new_season: u32,
        new_episode: u32,
    ) -> anyhow::Result<PathBuf> {
        if !exists(current_path).await {
            return Err(anyhow::anyhow!(
                "Current file does not exist: {}",
                current_path.display()
            ));
        }

        let new_target = self
            .library_dir
            .join(Self::sanitize_filename(new_anime_title))
            .join(format!("Season {:02}", new_season));

        fs::create_dir_all(&new_target).await?;

        let extension = current_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");

        let new_filename = format!(
            "{} - S{:02}E{:02}.{}",
            Self::sanitize_filename(new_anime_title),
            new_season,
            new_episode,
            extension
        );

        let new_path = new_target.join(new_filename);

        if new_path == current_path {
            return Ok(new_path);
        }

        move_file(current_path, &new_path).await?;
        self.move_companions(current_path, &new_path).await;
        // 舊的單集 NFO 會依新內容重新產生
        let _ = fs::remove_file(current_path.with_extension("nfo")).await;

        tracing::info!(
            "Resync moved: {} -> {}",
            current_path.display(),
            new_path.display()
        );

        // Clean up empty parent directories
        self.cleanup_empty_dirs(current_path).await;

        Ok(new_path)
    }

    /// 跟著影片搬走同名的其他檔案（字幕等，`{舊檔名}.zh-TW.ass` → `{新檔名}.zh-TW.ass`）。
    /// 單集 NFO 不搬，會重新產生。
    async fn move_companions(&self, old_video: &Path, new_video: &Path) {
        let (Some(dir), Some(old_stem), Some(new_stem)) = (
            old_video.parent(),
            old_video.file_stem().and_then(|s| s.to_str()),
            new_video.file_stem().and_then(|s| s.to_str()),
        ) else {
            return;
        };
        let prefix = format!("{}.", old_stem);
        let Ok(mut entries) = fs::read_dir(dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(suffix) = name.strip_prefix(&prefix) else {
                continue;
            };
            if suffix.eq_ignore_ascii_case("nfo") {
                continue;
            }
            let target = new_video.with_file_name(format!("{}.{}", new_stem, suffix));
            match move_file(&entry.path(), &target).await {
                Ok(()) => tracing::info!("Moved companion {} -> {}", name, target.display()),
                Err(e) => tracing::warn!("Failed to move companion {}: {}", name, e),
            }
        }
    }

    /// 搬走檔案後清理舊的季資料夾與系列資料夾：只剩我們產生的 NFO 與海報時連同它們一起刪除；
    /// 還有其他檔案（別的集數、使用者的檔案）就保留整個資料夾。
    pub async fn cleanup_empty_dirs(&self, old_file_path: &Path) {
        let Some(season_dir) = old_file_path.parent() else {
            return;
        };
        if season_dir == self.library_dir || !remove_if_only_generated(season_dir).await {
            return;
        }
        if let Some(anime_dir) = season_dir.parent() {
            if anime_dir != self.library_dir {
                remove_if_only_generated(anime_dir).await;
            }
        }
    }

    pub fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn extract_episode_info(&self, filename: &str) -> Option<(u32, u32)> {
        if let Some(caps) = EPISODE_REGEX.captures(filename) {
            // Format: S##E##
            if let (Some(season_match), Some(episode_match)) = (caps.get(1), caps.get(2)) {
                if let (Ok(season), Ok(episode)) = (
                    season_match.as_str().parse::<u32>(),
                    episode_match.as_str().parse::<u32>(),
                ) {
                    return Some((season, episode));
                }
            }
            // Format: [##] (simplified format)
            if let Some(episode_match) = caps.get(3) {
                if let Ok(episode) = episode_match.as_str().parse::<u32>() {
                    return Some((1, episode));
                }
            }
        }
        None
    }

    pub fn get_source_dir(&self) -> &Path {
        &self.source_dir
    }

    pub fn get_library_dir(&self) -> &Path {
        &self.library_dir
    }

    /// 建構字幕檔的目標檔名。
    /// 例：source="/downloads/sub.TC.ass", title="Title", season=1, episode=1
    ///     map 含 TC→zh-TW → "Title - S01E01.zh-TW.ass"
    /// 若無語言 tag → "Title - S01E01.ass"
    pub(crate) fn build_subtitle_dest_name(
        title: &str,
        season: u32,
        episode: u32,
        source_path: &str,
        language_codes: &shared::LanguageCodeMap,
    ) -> String {
        let path = std::path::Path::new(source_path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("ass");

        let base = format!(
            "{} - S{:02}E{:02}",
            Self::sanitize_filename(title),
            season,
            episode
        );

        match shared::extract_language_tag(source_path) {
            Some(raw_tag) => {
                let normalized = language_codes.normalize(&raw_tag);
                format!("{}.{}.{}", base, normalized, ext)
            }
            None => format!("{}.{}", base, ext),
        }
    }

    /// 搬移所有字幕檔到 Jellyfin library 的對應位置。
    /// 字幕命名規則：{title} - SxxExx.{lang}.{ext}
    /// 若有重複的語言 tag，加上序號（.1, .2 ...）。
    pub async fn organize_subtitles(
        &self,
        subtitle_paths: &[String],
        anime_title: &str,
        season: u32,
        episode: u32,
    ) -> Vec<PathBuf> {
        use std::collections::HashSet;

        let mut results = Vec::new();
        let season_dir = self
            .library_dir
            .join(Self::sanitize_filename(anime_title))
            .join(format!("Season {:02}", season));

        let mut used_names: HashSet<String> = HashSet::new();

        // 確保目標目錄存在（organize_episode 通常已建立，但防禦性建立）
        if let Err(e) = tokio::fs::create_dir_all(&season_dir).await {
            tracing::warn!("Failed to create season dir for subtitles: {}", e);
            return results;
        }

        for source_path in subtitle_paths {
            let source = self.resolve_download_path(source_path).await;
            if !exists(&source).await {
                tracing::warn!("Subtitle file not found: {}", source_path);
                continue;
            }

            let mut dest_name = Self::build_subtitle_dest_name(
                anime_title,
                season,
                episode,
                source_path,
                &self.language_codes,
            );

            // 處理重複檔名
            if used_names.contains(&dest_name) {
                let ext = std::path::Path::new(&dest_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ass")
                    .to_string();
                let stem = dest_name.trim_end_matches(&format!(".{}", ext));
                let mut i = 2;
                loop {
                    let candidate = format!("{}.{}.{}", stem, i, ext);
                    if !used_names.contains(&candidate) {
                        dest_name = candidate;
                        break;
                    }
                    i += 1;
                }
            }
            used_names.insert(dest_name.clone());

            let dest = season_dir.join(&dest_name);

            // 嘗試 rename，失敗時 copy+delete（cross-device fallback）
            match tokio::fs::rename(&source, &dest).await {
                Ok(()) => {
                    tracing::info!("Moved subtitle: {} → {}", source.display(), dest.display());
                    results.push(dest);
                }
                Err(_) => match tokio::fs::copy(&source, &dest).await {
                    Ok(_) => {
                        let _ = tokio::fs::remove_file(&source).await;
                        results.push(dest);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to move subtitle {}: {}", source_path, e);
                    }
                },
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            FileOrganizer::sanitize_filename("Test: Anime / Title"),
            "Test_ Anime _ Title"
        );
        assert_eq!(
            FileOrganizer::sanitize_filename("Attack*on*Titan?"),
            "Attack_on_Titan_"
        );
        assert_eq!(
            FileOrganizer::sanitize_filename("Demon<Slayer>"),
            "Demon_Slayer_"
        );
    }

    #[test]
    fn test_extract_episode_info_s_e_format() {
        let organizer = FileOrganizer::new(
            PathBuf::from("/downloads"),
            PathBuf::from("/media/jellyfin"),
            shared::LanguageCodeMap::default(),
        );

        assert_eq!(
            organizer.extract_episode_info("anime_s01e01.mkv"),
            Some((1, 1))
        );
        assert_eq!(
            organizer.extract_episode_info("anime_S05E12.mkv"),
            Some((5, 12))
        );
        assert_eq!(
            organizer.extract_episode_info("Episode_S02E03.mp4"),
            Some((2, 3))
        );
    }

    #[test]
    fn test_extract_episode_info_bracket_format() {
        let organizer = FileOrganizer::new(
            PathBuf::from("/downloads"),
            PathBuf::from("/media/jellyfin"),
            shared::LanguageCodeMap::default(),
        );

        assert_eq!(
            organizer.extract_episode_info("anime_[01].mkv"),
            Some((1, 1))
        );
        assert_eq!(
            organizer.extract_episode_info("anime_[12].mkv"),
            Some((1, 12))
        );
    }

    #[test]
    fn test_extract_episode_info_no_match() {
        let organizer = FileOrganizer::new(
            PathBuf::from("/downloads"),
            PathBuf::from("/media/jellyfin"),
            shared::LanguageCodeMap::default(),
        );

        assert_eq!(organizer.extract_episode_info("random_file.mkv"), None);
        assert_eq!(organizer.extract_episode_info("episode.txt"), None);
    }

    #[test]
    fn test_file_organizer_creation() {
        let organizer = FileOrganizer::new(
            PathBuf::from("/downloads"),
            PathBuf::from("/media/jellyfin"),
            shared::LanguageCodeMap::default(),
        );

        assert_eq!(organizer.get_source_dir(), Path::new("/downloads"));
        assert_eq!(organizer.get_library_dir(), Path::new("/media/jellyfin"));
    }

    #[test]
    fn test_build_subtitle_dest_name_with_lang() {
        let map =
            shared::LanguageCodeMap::from_entries(vec![("TC".to_string(), "zh-TW".to_string())]);
        let name =
            FileOrganizer::build_subtitle_dest_name("Title", 1, 1, "/downloads/sub.TC.ass", &map);
        assert_eq!(name, "Title - S01E01.zh-TW.ass");
    }

    #[test]
    fn test_build_subtitle_dest_name_no_lang() {
        let map = shared::LanguageCodeMap::from_entries(vec![]);
        let name =
            FileOrganizer::build_subtitle_dest_name("Title", 1, 1, "/downloads/subtitle.ass", &map);
        assert_eq!(name, "Title - S01E01.ass");
    }

    #[test]
    fn test_build_subtitle_dest_name_unknown_lang() {
        let map = shared::LanguageCodeMap::from_entries(vec![]);
        let name =
            FileOrganizer::build_subtitle_dest_name("Title", 1, 1, "/downloads/sub.XX.srt", &map);
        assert_eq!(name, "Title - S01E01.XX.srt");
    }

    fn organizer_in(root: &Path) -> FileOrganizer {
        FileOrganizer::new(
            root.join("downloads"),
            root.join("library"),
            shared::LanguageCodeMap::default(),
        )
    }

    #[tokio::test]
    async fn repeated_sync_reports_the_already_organized_file() {
        let root = tempfile::tempdir().unwrap();
        let organizer = organizer_in(root.path());
        let source = root.path().join("downloads/[Group] Show - 03.mkv");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"video").unwrap();

        let first = organizer
            .organize_episode("Show", 1, 3, &source)
            .await
            .unwrap();
        assert!(!source.exists());

        // E6: a duplicate request arrives after the first one moved the file
        let second = organizer
            .organize_episode("Show", 1, 3, &source)
            .await
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read(&second).unwrap(), b"video");
    }

    #[tokio::test]
    async fn missing_source_without_organized_file_still_fails() {
        let root = tempfile::tempdir().unwrap();
        let organizer = organizer_in(root.path());
        let source = root.path().join("downloads/[Group] Show - 04.mkv");

        let err = organizer
            .organize_episode("Show", 1, 4, &source)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Source file does not exist"));
    }

    fn touch(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn a_moved_episode_takes_its_subtitles_along() {
        let root = tempfile::tempdir().unwrap();
        let organizer = organizer_in(root.path());
        let old = root.path().join("library/舊名/Season 01/舊名 - S01E01.mkv");
        touch(&old, "video");
        touch(&old.with_file_name("舊名 - S01E01.zh-TW.ass"), "tw");
        touch(
            &old.with_file_name("舊名 - S01E02.zh-TW.ass"),
            "other episode",
        );

        let new = organizer.move_episode(&old, "新名", 1, 1).await.unwrap();

        let season = root.path().join("library/新名/Season 01");
        assert_eq!(new, season.join("新名 - S01E01.mkv"));
        assert_eq!(
            std::fs::read_to_string(season.join("新名 - S01E01.zh-TW.ass")).unwrap(),
            "tw"
        );
        // 別集的字幕不動
        assert!(old.with_file_name("舊名 - S01E02.zh-TW.ass").exists());
    }

    #[tokio::test]
    async fn moving_the_last_episode_out_removes_the_generated_files_and_folders() {
        let root = tempfile::tempdir().unwrap();
        let organizer = organizer_in(root.path());
        let series = root.path().join("library/舊名");
        let old = series.join("Season 01/舊名 - S01E01.mkv");
        touch(&old, "video");
        touch(&old.with_extension("nfo"), "ep nfo");
        touch(&series.join("Season 01/season.nfo"), "season");
        touch(&series.join("Season 01/poster.jpg"), "poster");
        touch(&series.join("tvshow.nfo"), "show");
        touch(&series.join("poster.jpg"), "poster");

        organizer.move_episode(&old, "新名", 1, 1).await.unwrap();

        assert!(!series.exists(), "old series folder should be gone");
    }

    #[tokio::test]
    async fn a_folder_with_other_episodes_is_kept_whole() {
        let root = tempfile::tempdir().unwrap();
        let organizer = organizer_in(root.path());
        let season = root.path().join("library/舊名/Season 01");
        let old = season.join("舊名 - S01E01.mkv");
        touch(&old, "video");
        touch(&season.join("舊名 - S01E02.mkv"), "another");
        touch(&season.join("season.nfo"), "season");

        organizer.move_episode(&old, "新名", 1, 1).await.unwrap();

        assert!(season.join("舊名 - S01E02.mkv").exists());
        assert!(season.join("season.nfo").exists());
    }

    #[tokio::test]
    async fn a_folder_with_unknown_files_is_not_removed() {
        let root = tempfile::tempdir().unwrap();
        let organizer = organizer_in(root.path());
        let season = root.path().join("library/舊名/Season 01");
        let old = season.join("舊名 - S01E01.mkv");
        touch(&old, "video");
        touch(&season.join("notes.txt"), "user file");

        organizer.move_episode(&old, "新名", 1, 1).await.unwrap();

        assert!(season.join("notes.txt").exists());
    }
}
