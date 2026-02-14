use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;

#[allow(dead_code)]
static EPISODE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)s(\d+)e(\d+)|\[(\d+)\]").unwrap());

#[derive(Clone, Debug)]
pub struct FileOrganizer {
    source_dir: PathBuf,
    library_dir: PathBuf,
}

impl FileOrganizer {
    pub fn new(source_dir: PathBuf, library_dir: PathBuf) -> Self {
        Self {
            source_dir,
            library_dir,
        }
    }

    /// 解析下載檔案路徑：將容器內部路徑（/downloads/...）對應到本地 source_dir
    ///
    /// - 正式環境：viewer 容器內 /downloads 已掛載，路徑直接可用
    /// - 開發環境：viewer 以 cargo run 執行，/downloads 不存在，
    ///   需對應到 DOWNLOADS_DIR（如 ./tmp/bangumi-downloads）
    pub fn resolve_download_path(&self, file_path: &str) -> PathBuf {
        let path = Path::new(file_path);
        if path.exists() {
            return path.to_path_buf();
        }
        // 嘗試將 /downloads/... 前綴替換為 source_dir
        if let Ok(relative) = path.strip_prefix("/downloads") {
            let mapped = self.source_dir.join(relative);
            if mapped.exists() {
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
        // Validate source file exists
        if !source_file.exists() {
            return Err(anyhow::anyhow!(
                "Source file does not exist: {}",
                source_file.display()
            ));
        }

        let season_dir = self
            .library_dir
            .join(Self::sanitize_filename(anime_title))
            .join(format!("Season {:02}", season));

        fs::create_dir_all(&season_dir).await?;

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

        // Move the file (rename for same filesystem, fallback to copy+delete)
        if let Err(_) = fs::rename(source_file, &target_path).await {
            fs::copy(source_file, &target_path).await?;
            let _ = fs::remove_file(source_file).await;
        }

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
        if !current_path.exists() {
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

        if let Err(_) = fs::rename(current_path, &new_path).await {
            fs::copy(current_path, &new_path).await?;
            let _ = fs::remove_file(current_path).await;
        }

        tracing::info!(
            "Resync moved: {} -> {}",
            current_path.display(),
            new_path.display()
        );

        // Clean up empty parent directories
        self.cleanup_empty_dirs(current_path).await;

        Ok(new_path)
    }

    /// Remove empty Season and anime directories after a file is moved out.
    async fn cleanup_empty_dirs(&self, old_file_path: &Path) {
        if let Some(season_dir) = old_file_path.parent() {
            if self.is_empty_dir(season_dir).await {
                let _ = fs::remove_dir(season_dir).await;
                tracing::info!("Removed empty directory: {}", season_dir.display());

                if let Some(anime_dir) = season_dir.parent() {
                    if anime_dir != self.library_dir && self.is_empty_dir(anime_dir).await {
                        let _ = fs::remove_dir(anime_dir).await;
                        tracing::info!("Removed empty directory: {}", anime_dir.display());
                    }
                }
            }
        }
    }

    async fn is_empty_dir(&self, dir: &Path) -> bool {
        match fs::read_dir(dir).await {
            Ok(mut entries) => entries.next_entry().await.ok().flatten().is_none(),
            Err(_) => false,
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
        );

        assert_eq!(organizer.extract_episode_info("random_file.mkv"), None);
        assert_eq!(organizer.extract_episode_info("episode.txt"), None);
    }

    #[test]
    fn test_file_organizer_creation() {
        let organizer = FileOrganizer::new(
            PathBuf::from("/downloads"),
            PathBuf::from("/media/jellyfin"),
        );

        assert_eq!(organizer.get_source_dir(), Path::new("/downloads"));
        assert_eq!(organizer.get_library_dir(), Path::new("/media/jellyfin"));
    }
}
