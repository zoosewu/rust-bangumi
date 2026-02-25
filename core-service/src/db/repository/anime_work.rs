use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{AnimeWork, NewAnimeWork};
use crate::schema::anime_works;

#[async_trait]
pub trait AnimeWorkRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeWork>, RepositoryError>;
    async fn find_by_title(&self, title: &str) -> Result<Option<AnimeWork>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<AnimeWork>, RepositoryError>;
    async fn create(&self, title: String) -> Result<AnimeWork, RepositoryError>;
    async fn update(&self, id: i32, title: String) -> Result<AnimeWork, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    async fn find_or_create(&self, title: String) -> Result<AnimeWork, RepositoryError>;
}

pub struct DieselAnimeWorkRepository {
    pool: DbPool,
}

impl DieselAnimeWorkRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnimeWorkRepository for DieselAnimeWorkRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeWork>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_works::table
                .find(id)
                .first::<AnimeWork>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_title(&self, title: &str) -> Result<Option<AnimeWork>, RepositoryError> {
        let pool = self.pool.clone();
        let title = title.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_works::table
                .filter(anime_works::title.eq(&title))
                .first::<AnimeWork>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<AnimeWork>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_works::table
                .load::<AnimeWork>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, title: String) -> Result<AnimeWork, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            let new_anime_work = NewAnimeWork {
                title,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(anime_works::table)
                .values(&new_anime_work)
                .get_result::<AnimeWork>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update(&self, id: i32, title: String) -> Result<AnimeWork, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(anime_works::table.find(id))
                .set((
                    anime_works::title.eq(&title),
                    anime_works::updated_at.eq(Utc::now().naive_utc()),
                ))
                .get_result::<AnimeWork>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let rows_deleted = diesel::delete(anime_works::table.find(id)).execute(&mut conn)?;
            Ok(rows_deleted > 0)
        })
        .await?
    }

    async fn find_or_create(&self, title: String) -> Result<AnimeWork, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            // Try to find existing
            match anime_works::table
                .filter(anime_works::title.eq(&title))
                .first::<AnimeWork>(&mut conn)
            {
                Ok(anime_work) => Ok(anime_work),
                Err(diesel::NotFound) => {
                    // Create new
                    let now = Utc::now().naive_utc();
                    let new_anime_work = NewAnimeWork {
                        title,
                        created_at: now,
                        updated_at: now,
                    };
                    diesel::insert_into(anime_works::table)
                        .values(&new_anime_work)
                        .get_result::<AnimeWork>(&mut conn)
                        .map_err(RepositoryError::from)
                }
                Err(e) => Err(RepositoryError::from(e)),
            }
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockAnimeWorkRepository {
        pub anime_works: Mutex<Vec<AnimeWork>>,
        pub operations: Mutex<Vec<String>>,
    }

    impl MockAnimeWorkRepository {
        pub fn new() -> Self {
            Self {
                anime_works: Mutex::new(Vec::new()),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(anime_works: Vec<AnimeWork>) -> Self {
            Self {
                anime_works: Mutex::new(anime_works),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockAnimeWorkRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl AnimeWorkRepository for MockAnimeWorkRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<AnimeWork>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .anime_works
                .lock()
                .unwrap()
                .iter()
                .find(|a| a.work_id == id)
                .cloned())
        }

        async fn find_by_title(&self, title: &str) -> Result<Option<AnimeWork>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_title:{}", title));
            Ok(self
                .anime_works
                .lock()
                .unwrap()
                .iter()
                .find(|a| a.title == title)
                .cloned())
        }

        async fn find_all(&self) -> Result<Vec<AnimeWork>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.anime_works.lock().unwrap().clone())
        }

        async fn create(&self, title: String) -> Result<AnimeWork, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("create:{}", title));
            let mut anime_works = self.anime_works.lock().unwrap();
            let id = anime_works.len() as i32 + 1;
            let now = Utc::now().naive_utc();
            let anime_work = AnimeWork {
                work_id: id,
                title,
                created_at: now,
                updated_at: now,
            };
            anime_works.push(anime_work.clone());
            Ok(anime_work)
        }

        async fn update(&self, id: i32, title: String) -> Result<AnimeWork, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("update:{}:{}", id, title));
            let mut anime_works = self.anime_works.lock().unwrap();
            if let Some(pos) = anime_works.iter().position(|a| a.work_id == id) {
                anime_works[pos].title = title;
                anime_works[pos].updated_at = Utc::now().naive_utc();
                Ok(anime_works[pos].clone())
            } else {
                Err(RepositoryError::NotFound)
            }
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("delete:{}", id));
            let mut anime_works = self.anime_works.lock().unwrap();
            let len_before = anime_works.len();
            anime_works.retain(|a| a.work_id != id);
            Ok(anime_works.len() < len_before)
        }

        async fn find_or_create(&self, title: String) -> Result<AnimeWork, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_or_create:{}", title));
            // Try to find existing
            {
                let anime_works = self.anime_works.lock().unwrap();
                if let Some(anime_work) = anime_works.iter().find(|a| a.title == title) {
                    return Ok(anime_work.clone());
                }
            }
            // Create new
            let mut anime_works = self.anime_works.lock().unwrap();
            let id = anime_works.len() as i32 + 1;
            let now = Utc::now().naive_utc();
            let anime_work = AnimeWork {
                work_id: id,
                title,
                created_at: now,
                updated_at: now,
            };
            anime_works.push(anime_work.clone());
            Ok(anime_work)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_mock_anime_work_repository_create() {
            let repo = MockAnimeWorkRepository::new();

            let anime_work = repo.create("Test Anime".to_string()).await.unwrap();
            assert_eq!(anime_work.work_id, 1);
            assert_eq!(anime_work.title, "Test Anime");

            let ops = repo.get_operations();
            assert_eq!(ops, vec!["create:Test Anime"]);
        }

        #[tokio::test]
        async fn test_mock_anime_work_repository_find_by_id() {
            let anime_work = AnimeWork {
                work_id: 1,
                title: "Existing Anime".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            let repo = MockAnimeWorkRepository::with_data(vec![anime_work]);

            let found = repo.find_by_id(1).await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().title, "Existing Anime");

            let not_found = repo.find_by_id(999).await.unwrap();
            assert!(not_found.is_none());
        }

        #[tokio::test]
        async fn test_mock_anime_work_repository_update() {
            let anime_work = AnimeWork {
                work_id: 1,
                title: "Original Title".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            let repo = MockAnimeWorkRepository::with_data(vec![anime_work]);

            let updated = repo.update(1, "New Title".to_string()).await.unwrap();
            assert_eq!(updated.title, "New Title");

            let result = repo.update(999, "Not Found".to_string()).await;
            assert!(matches!(result, Err(RepositoryError::NotFound)));
        }

        #[tokio::test]
        async fn test_mock_anime_work_repository_delete() {
            let anime_work = AnimeWork {
                work_id: 1,
                title: "To Delete".to_string(),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };
            let repo = MockAnimeWorkRepository::with_data(vec![anime_work]);

            let deleted = repo.delete(1).await.unwrap();
            assert!(deleted);

            let not_deleted = repo.delete(999).await.unwrap();
            assert!(!not_deleted);
        }
    }
}
