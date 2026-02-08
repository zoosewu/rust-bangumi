use crate::bangumi_client::{EpisodeItem, SubjectDetail};
use std::path::Path;
use tokio::fs;

/// Generate tvshow.nfo in the anime root directory
pub async fn generate_tvshow_nfo(anime_dir: &Path, subject: &SubjectDetail) -> anyhow::Result<()> {
    let nfo_path = anime_dir.join("tvshow.nfo");

    // Skip if already exists
    if nfo_path.exists() {
        return Ok(());
    }

    let title = xml_escape(&subject.name);
    let title_cn = subject
        .name_cn
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let plot = subject
        .summary
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let rating = subject
        .rating
        .as_ref()
        .and_then(|r| r.score)
        .map(|s| format!("{:.1}", s))
        .unwrap_or_default();
    let year = subject
        .date
        .as_deref()
        .and_then(|d| d.split('-').next())
        .unwrap_or("");

    let nfo_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<tvshow>
    <title>{title_cn}</title>
    <originaltitle>{title}</originaltitle>
    <plot>{plot}</plot>
    <rating>{rating}</rating>
    <year>{year}</year>
    <uniqueid type="bangumi">{bangumi_id}</uniqueid>
</tvshow>
"#,
        title_cn = if title_cn.is_empty() {
            &title
        } else {
            &title_cn
        },
        title = title,
        plot = plot,
        rating = rating,
        year = year,
        bangumi_id = subject.id,
    );

    fs::write(&nfo_path, nfo_content).await?;
    tracing::info!("Generated tvshow.nfo at {}", nfo_path.display());
    Ok(())
}

/// Generate episode NFO file next to the video file
pub async fn generate_episode_nfo(
    video_path: &Path,
    episode: &EpisodeItem,
    season: i32,
) -> anyhow::Result<()> {
    let nfo_path = video_path.with_extension("nfo");

    let title = episode.name.as_deref().map(xml_escape).unwrap_or_default();
    let title_cn = episode
        .name_cn
        .as_deref()
        .map(xml_escape)
        .unwrap_or_default();
    let aired = episode.airdate.as_deref().unwrap_or("");
    let plot = episode.desc.as_deref().map(xml_escape).unwrap_or_default();
    let ep_no = episode.ep.unwrap_or(episode.sort);

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
    <uniqueid type="bangumi">{bangumi_ep_id}</uniqueid>
</episodedetails>
"#,
        title = display_title,
        season = season,
        episode = ep_no,
        aired = aired,
        plot = plot,
        bangumi_ep_id = episode.id,
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

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("foo & bar"), "foo &amp; bar");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape(r#"a"b'c"#), "a&quot;b&apos;c");
    }
}
