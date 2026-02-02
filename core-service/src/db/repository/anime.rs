use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::{Anime, NewAnime};
use crate::schema::animes;
use super::RepositoryError;

#[async_trait]
pub trait AnimeRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Anime>, RepositoryError>;
    async fn find_by_title(&self, title: &str) -> Result<Option<Anime>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<Anime>, RepositoryError>;
    async fn create(&self, title: String) -> Result<Anime, RepositoryError>;
    async fn update(&self, id: i32, title: String) -> Result<Anime, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
}

pub struct DieselAnimeRepository {
    pool: DbPool,
}

impl DieselAnimeRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnimeRepository for DieselAnimeRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<Anime>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            animes::table
                .find(id)
                .first::<Anime>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_title(&self, title: &str) -> Result<Option<Anime>, RepositoryError> {
        let pool = self.pool.clone();
        let title = title.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            animes::table
                .filter(animes::title.eq(&title))
                .first::<Anime>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<Anime>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            animes::table
                .load::<Anime>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, title: String) -> Result<Anime, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            let new_anime = NewAnime {
                title,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(animes::table)
                .values(&new_anime)
                .get_result::<Anime>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update(&self, id: i32, title: String) -> Result<Anime, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(animes::table.find(id))
                .set((
                    animes::title.eq(&title),
                    animes::updated_at.eq(Utc::now().naive_utc()),
                ))
                .get_result::<Anime>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let rows_deleted = diesel::delete(animes::table.find(id))
                .execute(&mut conn)?;
            Ok(rows_deleted > 0)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockAnimeRepository {
        pub animes: Mutex<Vec<Anime>>,
        pub operations: Mutex<Vec<String>>,
    }

    impl MockAnimeRepository {
        pub fn new() -> Self {
            Self {
                animes: Mutex::new(Vec::new()),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(animes: Vec<Anime>) -> Self {
            Self {
                animes: Mutex::new(animes),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockAnimeRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl AnimeRepository for MockAnimeRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<Anime>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_id:{}", id));
            Ok(self.animes.lock().unwrap()
                .iter()
                .find(|a| a.anime_id == id)
                .cloned())
        }

        async fn find_by_title(&self, title: &str) -> Result<Option<Anime>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_title:{}", title));
            Ok(self.animes.lock().unwrap()
                .iter()
                .find(|a| a.title == title)
                .cloned())
        }

        async fn find_all(&self) -> Result<Vec<Anime>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.animes.lock().unwrap().clone())
        }

        async fn create(&self, title: String) -> Result<Anime, RepositoryError> {
            self.operations.lock().unwrap().push(format!("create:{}", title));
            let mut animes = self.animes.lock().unwrap();
            let id = animes.len() as i32 + 1;
            let now = Utc::now().naive_utc();
            let anime = Anime {
                anime_id: id,
                title,
                created_at: now,
                updated_at: now,
            };
            animes.push(anime.clone());
            Ok(anime)
        }

        async fn update(&self, id: i32, title: String) -> Result<Anime, RepositoryError> {
            self.operations.lock().unwrap().push(format!("update:{}:{}", id, title));
            let mut animes = self.animes.lock().unwrap();
            if let Some(pos) = animes.iter().position(|a| a.anime_id == id) {
                animes[pos].title = title;
                animes[pos].updated_at = Utc::now().naive_utc();
                Ok(animes[pos].clone())
            } else {
                Err(RepositoryError::NotFound)
            }
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations.lock().unwrap().push(format!("delete:{}", id));
            let mut animes = self.animes.lock().unwrap();
            let len_before = animes.len();
            animes.retain(|a| a.anime_id != id);
            Ok(animes.len() < len_before)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_mock_anime_repository_create() {
            let repo = MockAnimeRepository::new();

            let anime = repo.create("Test Anime".to_string()).await.unwrap();
            assert_eq!(anime.anime_id, 1);
            assert_eq!(anime.title, "Test Anime");

            let ops = repo.get_operations();
            assert_eq!(ops, vec!["create:Test Anime"]);
        }

        #[tokio::test]
        async fn test_mock_anime_repository_find_by_id() {
            let anime = Anime {
                anime_id: 1,
                title: "Existing Anime".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            let repo = MockAnimeRepository::with_data(vec![anime]);

            let found = repo.find_by_id(1).await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().title, "Existing Anime");

            let not_found = repo.find_by_id(999).await.unwrap();
            assert!(not_found.is_none());
        }

        #[tokio::test]
        async fn test_mock_anime_repository_update() {
            let anime = Anime {
                anime_id: 1,
                title: "Original Title".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            let repo = MockAnimeRepository::with_data(vec![anime]);

            let updated = repo.update(1, "New Title".to_string()).await.unwrap();
            assert_eq!(updated.title, "New Title");

            let result = repo.update(999, "Not Found".to_string()).await;
            assert!(matches!(result, Err(RepositoryError::NotFound)));
        }

        #[tokio::test]
        async fn test_mock_anime_repository_delete() {
            let anime = Anime {
                anime_id: 1,
                title: "To Delete".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            let repo = MockAnimeRepository::with_data(vec![anime]);

            let deleted = repo.delete(1).await.unwrap();
            assert!(deleted);

            let not_deleted = repo.delete(999).await.unwrap();
            assert!(!not_deleted);
        }
    }
}
