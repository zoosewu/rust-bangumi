use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{Anime, NewAnime};
use crate::schema::animes;

#[derive(Debug, Clone)]
pub struct CreateAnimeParams {
    pub work_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[async_trait]
pub trait AnimeRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Anime>, RepositoryError>;
    async fn find_by_work_id(&self, work_id: i32) -> Result<Vec<Anime>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<Anime>, RepositoryError>;
    async fn create(&self, params: CreateAnimeParams) -> Result<Anime, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    async fn find_or_create(
        &self,
        work_id: i32,
        series_no: i32,
        season_id: i32,
        description: Option<String>,
    ) -> Result<Anime, RepositoryError>;
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
                .filter(animes::anime_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_work_id(&self, work_id: i32) -> Result<Vec<Anime>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            animes::table
                .filter(animes::work_id.eq(work_id))
                .load::<Anime>(&mut conn)
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

    async fn create(&self, params: CreateAnimeParams) -> Result<Anime, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            let new_anime = NewAnime {
                work_id: params.work_id,
                series_no: params.series_no,
                season_id: params.season_id,
                description: params.description,
                aired_date: params.aired_date,
                end_date: params.end_date,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(animes::table)
                .values(&new_anime)
                .get_result(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let deleted =
                diesel::delete(animes::table.filter(animes::anime_id.eq(id)))
                    .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }

    async fn find_or_create(
        &self,
        work_id: i32,
        series_no: i32,
        season_id: i32,
        description: Option<String>,
    ) -> Result<Anime, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            // Try to find existing
            match animes::table
                .filter(animes::work_id.eq(work_id))
                .filter(animes::series_no.eq(series_no))
                .filter(animes::season_id.eq(season_id))
                .first::<Anime>(&mut conn)
            {
                Ok(s) => Ok(s),
                Err(diesel::NotFound) => {
                    // Create new
                    let now = Utc::now().naive_utc();
                    let new_anime = NewAnime {
                        work_id,
                        series_no,
                        season_id,
                        description,
                        aired_date: None,
                        end_date: None,
                        created_at: now,
                        updated_at: now,
                    };
                    diesel::insert_into(animes::table)
                        .values(&new_anime)
                        .get_result(&mut conn)
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

    pub struct MockAnimeRepository {
        series: Mutex<Vec<Anime>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockAnimeRepository {
        pub fn new() -> Self {
            Self {
                series: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(series: Vec<Anime>) -> Self {
            let max_id = series.iter().map(|s| s.anime_id).max().unwrap_or(0);
            Self {
                series: Mutex::new(series),
                next_id: Mutex::new(max_id + 1),
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
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .series
                .lock()
                .unwrap()
                .iter()
                .find(|s| s.anime_id == id)
                .cloned())
        }

        async fn find_by_work_id(
            &self,
            work_id: i32,
        ) -> Result<Vec<Anime>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_work_id:{}", work_id));
            Ok(self
                .series
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.work_id == work_id)
                .cloned()
                .collect())
        }

        async fn find_all(&self) -> Result<Vec<Anime>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.series.lock().unwrap().clone())
        }

        async fn create(
            &self,
            params: CreateAnimeParams,
        ) -> Result<Anime, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("create:work_id:{}", params.work_id));
            let mut series = self.series.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_anime = Anime {
                anime_id: *next_id,
                work_id: params.work_id,
                series_no: params.series_no,
                season_id: params.season_id,
                description: params.description,
                aired_date: params.aired_date,
                end_date: params.end_date,
                created_at: now,
                updated_at: now,
            };
            *next_id += 1;
            series.push(new_anime.clone());
            Ok(new_anime)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("delete:{}", id));
            let mut series = self.series.lock().unwrap();
            let original_len = series.len();
            series.retain(|s| s.anime_id != id);
            Ok(series.len() < original_len)
        }

        async fn find_or_create(
            &self,
            work_id: i32,
            series_no: i32,
            season_id: i32,
            description: Option<String>,
        ) -> Result<Anime, RepositoryError> {
            self.operations.lock().unwrap().push(format!(
                "find_or_create:{}:{}:{}",
                work_id, series_no, season_id
            ));
            // Try to find existing
            {
                let series = self.series.lock().unwrap();
                if let Some(s) = series.iter().find(|s| {
                    s.work_id == work_id && s.series_no == series_no && s.season_id == season_id
                }) {
                    return Ok(s.clone());
                }
            }
            // Create new
            let mut series = self.series.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_anime = Anime {
                anime_id: *next_id,
                work_id,
                series_no,
                season_id,
                description,
                aired_date: None,
                end_date: None,
                created_at: now,
                updated_at: now,
            };
            *next_id += 1;
            series.push(new_anime.clone());
            Ok(new_anime)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockAnimeRepository;
    use super::*;

    #[tokio::test]
    async fn test_mock_anime_repository_create() {
        let repo = MockAnimeRepository::new();
        let params = CreateAnimeParams {
            work_id: 1,
            series_no: 1,
            season_id: 1,
            description: Some("Test description".to_string()),
            aired_date: None,
            end_date: None,
        };
        let anime = repo.create(params).await.unwrap();

        assert_eq!(anime.anime_id, 1);
        assert_eq!(anime.work_id, 1);
        assert_eq!(anime.series_no, 1);

        let ops = repo.get_operations();
        assert!(ops.contains(&"create:work_id:1".to_string()));
    }

    #[tokio::test]
    async fn test_mock_anime_repository_find_by_id() {
        let anime = Anime {
            anime_id: 1,
            work_id: 1,
            series_no: 1,
            season_id: 1,
            description: Some("Test".to_string()),
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeRepository::with_data(vec![anime]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().anime_id, 1);

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_mock_anime_repository_find_by_work_id() {
        let anime1 = Anime {
            anime_id: 1,
            work_id: 1,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let anime2 = Anime {
            anime_id: 2,
            work_id: 1,
            series_no: 2,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let anime3 = Anime {
            anime_id: 3,
            work_id: 2,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeRepository::with_data(vec![anime1, anime2, anime3]);

        let work1_animes = repo.find_by_work_id(1).await.unwrap();
        assert_eq!(work1_animes.len(), 2);

        let work2_animes = repo.find_by_work_id(2).await.unwrap();
        assert_eq!(work2_animes.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_anime_repository_delete() {
        let anime = Anime {
            anime_id: 1,
            work_id: 1,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
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
