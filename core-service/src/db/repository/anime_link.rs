use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::Int4;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{AnimeLink, NewAnimeLink};
use crate::schema::anime_links;

#[derive(QueryableByName, Debug)]
struct ConflictGroup {
    #[diesel(sql_type = Int4)]
    series_id: i32,
    #[diesel(sql_type = Int4)]
    group_id: i32,
    #[diesel(sql_type = Int4)]
    episode_no: i32,
}

#[async_trait]
pub trait AnimeLinkRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeLink>, RepositoryError>;
    async fn find_by_series_id(
        &self,
        series_id: i32,
        include_filtered: bool,
    ) -> Result<Vec<AnimeLink>, RepositoryError>;
    async fn create(&self, link: NewAnimeLink) -> Result<AnimeLink, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    /// Find all active (series_id, group_id, episode_no) groups that have COUNT > 1
    async fn detect_all_conflicts(&self) -> Result<Vec<(i32, i32, i32)>, RepositoryError>;
    /// Find all active link_ids for a given (series_id, group_id, episode_no)
    async fn find_active_links_for_episode(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<Vec<AnimeLink>, RepositoryError>;
    /// Batch set conflict_flag for given link_ids
    async fn set_conflict_flags(
        &self,
        link_ids: &[i32],
        flag: bool,
    ) -> Result<(), RepositoryError>;
    /// Batch set link_status for given link_ids
    async fn set_link_status(
        &self,
        link_ids: &[i32],
        status: &str,
    ) -> Result<(), RepositoryError>;
    /// Clear conflict_flag for all links (reset before re-detection)
    async fn clear_all_conflict_flags(&self) -> Result<usize, RepositoryError>;
    /// Find resolved (link_status='resolved') unfiltered links for a given episode
    async fn find_resolved_links_for_episode(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<Vec<AnimeLink>, RepositoryError>;
}

pub struct DieselAnimeLinkRepository {
    pool: DbPool,
}

impl DieselAnimeLinkRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnimeLinkRepository for DieselAnimeLinkRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeLink>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_links::table
                .filter(anime_links::link_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_series_id(
        &self,
        series_id: i32,
        include_filtered: bool,
    ) -> Result<Vec<AnimeLink>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            if include_filtered {
                anime_links::table
                    .filter(anime_links::series_id.eq(series_id))
                    .load::<AnimeLink>(&mut conn)
                    .map_err(RepositoryError::from)
            } else {
                anime_links::table
                    .filter(anime_links::series_id.eq(series_id))
                    .filter(anime_links::filtered_flag.eq(false))
                    .load::<AnimeLink>(&mut conn)
                    .map_err(RepositoryError::from)
            }
        })
        .await?
    }

    async fn create(&self, link: NewAnimeLink) -> Result<AnimeLink, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(anime_links::table)
                .values(&link)
                .get_result(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let deleted = diesel::delete(anime_links::table.filter(anime_links::link_id.eq(id)))
                .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }

    async fn detect_all_conflicts(&self) -> Result<Vec<(i32, i32, i32)>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            // Use raw SQL for GROUP BY + HAVING
            let results: Vec<(i32, i32, i32)> = diesel::sql_query(
                "SELECT series_id, group_id, episode_no \
                 FROM anime_links \
                 WHERE link_status = 'active' AND filtered_flag = false \
                 GROUP BY series_id, group_id, episode_no \
                 HAVING COUNT(*) > 1"
            )
            .load::<ConflictGroup>(&mut conn)?
            .into_iter()
            .map(|cg| (cg.series_id, cg.group_id, cg.episode_no))
            .collect();
            Ok(results)
        })
        .await?
    }

    async fn find_active_links_for_episode(
        &self,
        sid: i32,
        gid: i32,
        ep: i32,
    ) -> Result<Vec<AnimeLink>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_links::table
                .filter(anime_links::series_id.eq(sid))
                .filter(anime_links::group_id.eq(gid))
                .filter(anime_links::episode_no.eq(ep))
                .filter(anime_links::link_status.eq("active"))
                .filter(anime_links::filtered_flag.eq(false))
                .load::<AnimeLink>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn set_conflict_flags(
        &self,
        link_ids: &[i32],
        flag: bool,
    ) -> Result<(), RepositoryError> {
        let pool = self.pool.clone();
        let ids = link_ids.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(anime_links::table.filter(anime_links::link_id.eq_any(&ids)))
                .set(anime_links::conflict_flag.eq(flag))
                .execute(&mut conn)?;
            Ok(())
        })
        .await?
    }

    async fn set_link_status(
        &self,
        link_ids: &[i32],
        status: &str,
    ) -> Result<(), RepositoryError> {
        let pool = self.pool.clone();
        let ids = link_ids.to_vec();
        let status = status.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(anime_links::table.filter(anime_links::link_id.eq_any(&ids)))
                .set(anime_links::link_status.eq(&status))
                .execute(&mut conn)?;
            Ok(())
        })
        .await?
    }

    async fn clear_all_conflict_flags(&self) -> Result<usize, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let updated = diesel::update(
                anime_links::table.filter(anime_links::conflict_flag.eq(true)),
            )
            .set(anime_links::conflict_flag.eq(false))
            .execute(&mut conn)?;
            Ok(updated)
        })
        .await?
    }

    async fn find_resolved_links_for_episode(
        &self,
        sid: i32,
        gid: i32,
        ep: i32,
    ) -> Result<Vec<AnimeLink>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_links::table
                .filter(anime_links::series_id.eq(sid))
                .filter(anime_links::group_id.eq(gid))
                .filter(anime_links::episode_no.eq(ep))
                .filter(anime_links::link_status.eq("resolved"))
                .filter(anime_links::filtered_flag.eq(false))
                .load::<AnimeLink>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockAnimeLinkRepository {
        links: Mutex<Vec<AnimeLink>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockAnimeLinkRepository {
        pub fn new() -> Self {
            Self {
                links: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(links: Vec<AnimeLink>) -> Self {
            let max_id = links.iter().map(|l| l.link_id).max().unwrap_or(0);
            Self {
                links: Mutex::new(links),
                next_id: Mutex::new(max_id + 1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockAnimeLinkRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl AnimeLinkRepository for MockAnimeLinkRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<AnimeLink>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .links
                .lock()
                .unwrap()
                .iter()
                .find(|l| l.link_id == id)
                .cloned())
        }

        async fn find_by_series_id(
            &self,
            series_id: i32,
            include_filtered: bool,
        ) -> Result<Vec<AnimeLink>, RepositoryError> {
            self.operations.lock().unwrap().push(format!(
                "find_by_series_id:{}:{}",
                series_id, include_filtered
            ));
            let links = self.links.lock().unwrap();
            let result: Vec<AnimeLink> = links
                .iter()
                .filter(|l| l.series_id == series_id && (include_filtered || !l.filtered_flag))
                .cloned()
                .collect();
            Ok(result)
        }

        async fn create(&self, link: NewAnimeLink) -> Result<AnimeLink, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("create:series_id:{}", link.series_id));
            let mut links = self.links.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let new_link = AnimeLink {
                link_id: *next_id,
                series_id: link.series_id,
                group_id: link.group_id,
                episode_no: link.episode_no,
                title: link.title,
                url: link.url,
                source_hash: link.source_hash,
                filtered_flag: link.filtered_flag,
                created_at: link.created_at,
                raw_item_id: link.raw_item_id,
                download_type: link.download_type,
                conflict_flag: link.conflict_flag,
                link_status: link.link_status,
            };
            *next_id += 1;
            links.push(new_link.clone());
            Ok(new_link)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("delete:{}", id));
            let mut links = self.links.lock().unwrap();
            let original_len = links.len();
            links.retain(|l| l.link_id != id);
            Ok(links.len() < original_len)
        }

        async fn detect_all_conflicts(&self) -> Result<Vec<(i32, i32, i32)>, RepositoryError> {
            Ok(vec![])
        }

        async fn find_active_links_for_episode(
            &self,
            _series_id: i32,
            _group_id: i32,
            _episode_no: i32,
        ) -> Result<Vec<AnimeLink>, RepositoryError> {
            Ok(vec![])
        }

        async fn set_conflict_flags(
            &self,
            _link_ids: &[i32],
            _flag: bool,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn set_link_status(
            &self,
            _link_ids: &[i32],
            _status: &str,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn find_resolved_links_for_episode(
            &self,
            _series_id: i32,
            _group_id: i32,
            _episode_no: i32,
        ) -> Result<Vec<AnimeLink>, RepositoryError> {
            Ok(vec![])
        }

        async fn clear_all_conflict_flags(&self) -> Result<usize, RepositoryError> {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockAnimeLinkRepository;
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_mock_anime_link_repository_create() {
        let repo = MockAnimeLinkRepository::new();
        let now = Utc::now().naive_utc();
        let link = NewAnimeLink {
            series_id: 1,
            group_id: 1,
            episode_no: 1,
            title: Some("Test Episode".to_string()),
            url: "http://example.com/ep1".to_string(),
            source_hash: "abc123".to_string(),
            filtered_flag: false,
            created_at: now,
            raw_item_id: None,
            download_type: Some("http".to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };
        let created = repo.create(link).await.unwrap();
        assert_eq!(created.link_id, 1);
        assert_eq!(created.episode_no, 1);
    }

    #[tokio::test]
    async fn test_mock_anime_link_repository_find_by_series_id() {
        let now = Utc::now().naive_utc();
        let link1 = AnimeLink {
            link_id: 1,
            series_id: 1,
            group_id: 1,
            episode_no: 1,
            title: Some("EP1".to_string()),
            url: "http://example.com/1".to_string(),
            source_hash: "hash1".to_string(),
            filtered_flag: false,
            created_at: now,
            raw_item_id: None,
            download_type: Some("http".to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };
        let link2 = AnimeLink {
            link_id: 2,
            series_id: 1,
            group_id: 1,
            episode_no: 2,
            title: Some("EP2".to_string()),
            url: "http://example.com/2".to_string(),
            source_hash: "hash2".to_string(),
            filtered_flag: true, // filtered
            created_at: now,
            raw_item_id: None,
            download_type: Some("http".to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };
        let repo = MockAnimeLinkRepository::with_data(vec![link1, link2]);

        // Without filtered
        let links = repo.find_by_series_id(1, false).await.unwrap();
        assert_eq!(links.len(), 1);

        // With filtered
        let all_links = repo.find_by_series_id(1, true).await.unwrap();
        assert_eq!(all_links.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_anime_link_repository_delete() {
        let now = Utc::now().naive_utc();
        let link = AnimeLink {
            link_id: 1,
            series_id: 1,
            group_id: 1,
            episode_no: 1,
            title: None,
            url: "http://example.com".to_string(),
            source_hash: "hash".to_string(),
            filtered_flag: false,
            created_at: now,
            raw_item_id: None,
            download_type: Some("http".to_string()),
            conflict_flag: false,
            link_status: "active".to_string(),
        };
        let repo = MockAnimeLinkRepository::with_data(vec![link]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
