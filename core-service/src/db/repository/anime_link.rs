use async_trait::async_trait;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{AnimeLink, NewAnimeLink};
use crate::schema::anime_links;

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
        };
        let repo = MockAnimeLinkRepository::with_data(vec![link]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
