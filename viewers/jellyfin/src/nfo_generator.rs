use std::path::Path;
use tokio::fs;

/// NFO 生成所需的集數資料（與 bangumi_client 解耦）
#[derive(Debug, Default)]
pub struct EpisodeNfoData {
    pub title: Option<String>,
    pub title_cn: Option<String>,
    pub air_date: Option<String>,
    pub summary: Option<String>,
}

/// Generate episode NFO file next to the video file
pub async fn generate_episode_nfo(
    video_path: &Path,
    episode: &EpisodeNfoData,
    episode_no: i32,
    season: i32,
) -> anyhow::Result<()> {
    let nfo_path = video_path.with_extension("nfo");

    let title = episode.title.as_deref().map(xml_escape).unwrap_or_default();
    let title_cn = episode
        .title_cn
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let aired = episode.air_date.as_deref().unwrap_or("");
    let plot = episode
        .summary
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();

    let display_title = if !title_cn.is_empty() {
        &title_cn
    } else if !title.is_empty() {
        &title
    } else {
        ""
    };

    let nfo_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<episodedetails>
    <title>{title}</title>
    <season>{season}</season>
    <episode>{episode}</episode>
    <aired>{aired}</aired>
    <plot>{plot}</plot>
</episodedetails>
"#,
        title = display_title,
        season = season,
        episode = episode_no,
        aired = aired,
        plot = plot,
    );

    fs::write(&nfo_path, nfo_content).await?;
    tracing::info!("Generated episode NFO at {}", nfo_path.display());
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("foo & bar"), "foo &amp; bar");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape(r#"a"b'c"#), "a&quot;b&apos;c");
    }

    #[tokio::test]
    async fn test_generate_episode_nfo_basic() {
        let dir = tempdir().unwrap();
        let video_path = dir.path().join("Anime - S01E01.mkv");

        let episode = EpisodeNfoData {
            title: Some("First Episode".to_string()),
            title_cn: Some("第一集".to_string()),
            air_date: Some("2024-01-01".to_string()),
            summary: Some("A great episode".to_string()),
        };

        generate_episode_nfo(&video_path, &episode, 1, 1).await.unwrap();

        let nfo_path = video_path.with_extension("nfo");
        assert!(nfo_path.exists());
        let content = std::fs::read_to_string(&nfo_path).unwrap();
        assert!(content.contains("<title>第一集</title>"));
        assert!(content.contains("<season>1</season>"));
        assert!(content.contains("<episode>1</episode>"));
        assert!(content.contains("<aired>2024-01-01</aired>"));
    }

    #[tokio::test]
    async fn test_generate_episode_nfo_no_cn_title() {
        let dir = tempdir().unwrap();
        let video_path = dir.path().join("Anime - S01E02.mkv");

        let episode = EpisodeNfoData {
            title: Some("Second Episode".to_string()),
            title_cn: None,
            air_date: None,
            summary: None,
        };

        generate_episode_nfo(&video_path, &episode, 2, 1).await.unwrap();

        let content = std::fs::read_to_string(video_path.with_extension("nfo")).unwrap();
        assert!(content.contains("<title>Second Episode</title>"));
    }

    #[tokio::test]
    async fn test_generate_episode_nfo_xml_special_chars() {
        let dir = tempdir().unwrap();
        let video_path = dir.path().join("Anime - S01E03.mkv");

        let episode = EpisodeNfoData {
            title: Some("Ep & <Test>".to_string()),
            title_cn: None,
            air_date: None,
            summary: None,
        };

        generate_episode_nfo(&video_path, &episode, 3, 1).await.unwrap();

        let content = std::fs::read_to_string(video_path.with_extension("nfo")).unwrap();
        assert!(content.contains("Ep &amp; &lt;Test&gt;"));
    }
}
