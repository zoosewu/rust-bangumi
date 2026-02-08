use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;

#[allow(dead_code)]
static EPISODE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)s(\d+)e(\d+)|\[(\d+)\]").unwrap());

#[derive(Clone, Debug)]
pub struct FileOrganizer {
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
