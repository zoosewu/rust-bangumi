use async_trait::async_trait;
use diesel::prelude::*;
use chrono::{NaiveDate, Utc};

use crate::db::DbPool;
use crate::models::{AnimeSeries, NewAnimeSeries};
use crate::schema::anime_series;
use super::RepositoryError;

#[derive(Debug, Clone)]
pub struct CreateAnimeSeriesParams {
    pub anime_id: i32,
    pub series_no: i32,
    pub season_id: i32,
    pub description: Option<String>,
    pub aired_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[async_trait]
pub trait AnimeSeriesRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeSeries>, RepositoryError>;
    async fn find_by_anime_id(&self, anime_id: i32) -> Result<Vec<AnimeSeries>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<AnimeSeries>, RepositoryError>;
    async fn create(&self, params: CreateAnimeSeriesParams) -> Result<AnimeSeries, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    async fn find_or_create(&self, anime_id: i32, series_no: i32, season_id: i32, description: Option<String>) -> Result<AnimeSeries, RepositoryError>;
}

pub struct DieselAnimeSeriesRepository {
    pool: DbPool,
}

impl DieselAnimeSeriesRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnimeSeriesRepository for DieselAnimeSeriesRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeSeries>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_series::table
                .filter(anime_series::series_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_anime_id(&self, anime_id: i32) -> Result<Vec<AnimeSeries>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_series::table
                .filter(anime_series::anime_id.eq(anime_id))
                .load::<AnimeSeries>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<AnimeSeries>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_series::table
                .load::<AnimeSeries>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, params: CreateAnimeSeriesParams) -> Result<AnimeSeries, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            let new_series = NewAnimeSeries {
                anime_id: params.anime_id,
                series_no: params.series_no,
                season_id: params.season_id,
                description: params.description,
                aired_date: params.aired_date,
                end_date: params.end_date,
                created_at: now,
                updated_at: now,
            };
            diesel::insert_into(anime_series::table)
                .values(&new_series)
                .get_result(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let deleted = diesel::delete(anime_series::table.filter(anime_series::series_id.eq(id)))
                .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }

    async fn find_or_create(&self, anime_id: i32, series_no: i32, season_id: i32, description: Option<String>) -> Result<AnimeSeries, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            // Try to find existing
            match anime_series::table
                .filter(anime_series::anime_id.eq(anime_id))
                .filter(anime_series::series_no.eq(series_no))
                .filter(anime_series::season_id.eq(season_id))
                .first::<AnimeSeries>(&mut conn)
            {
                Ok(s) => Ok(s),
                Err(diesel::NotFound) => {
                    // Create new
                    let now = Utc::now().naive_utc();
                    let new_series = NewAnimeSeries {
                        anime_id,
                        series_no,
                        season_id,
                        description,
                        aired_date: None,
                        end_date: None,
                        created_at: now,
                        updated_at: now,
                    };
                    diesel::insert_into(anime_series::table)
                        .values(&new_series)
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

    pub struct MockAnimeSeriesRepository {
        series: Mutex<Vec<AnimeSeries>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockAnimeSeriesRepository {
        pub fn new() -> Self {
            Self {
                series: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(series: Vec<AnimeSeries>) -> Self {
            let max_id = series.iter().map(|s| s.series_id).max().unwrap_or(0);
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

    impl Default for MockAnimeSeriesRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl AnimeSeriesRepository for MockAnimeSeriesRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<AnimeSeries>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_id:{}", id));
            Ok(self.series.lock().unwrap().iter().find(|s| s.series_id == id).cloned())
        }

        async fn find_by_anime_id(&self, anime_id: i32) -> Result<Vec<AnimeSeries>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_anime_id:{}", anime_id));
            Ok(self.series.lock().unwrap().iter().filter(|s| s.anime_id == anime_id).cloned().collect())
        }

        async fn find_all(&self) -> Result<Vec<AnimeSeries>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.series.lock().unwrap().clone())
        }

        async fn create(&self, params: CreateAnimeSeriesParams) -> Result<AnimeSeries, RepositoryError> {
            self.operations.lock().unwrap().push(format!("create:anime_id:{}", params.anime_id));
            let mut series = self.series.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_series = AnimeSeries {
                series_id: *next_id,
                anime_id: params.anime_id,
                series_no: params.series_no,
                season_id: params.season_id,
                description: params.description,
                aired_date: params.aired_date,
                end_date: params.end_date,
                created_at: now,
                updated_at: now,
            };
            *next_id += 1;
            series.push(new_series.clone());
            Ok(new_series)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations.lock().unwrap().push(format!("delete:{}", id));
            let mut series = self.series.lock().unwrap();
            let original_len = series.len();
            series.retain(|s| s.series_id != id);
            Ok(series.len() < original_len)
        }

        async fn find_or_create(&self, anime_id: i32, series_no: i32, season_id: i32, description: Option<String>) -> Result<AnimeSeries, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_or_create:{}:{}:{}", anime_id, series_no, season_id));
            // Try to find existing
            {
                let series = self.series.lock().unwrap();
                if let Some(s) = series.iter().find(|s| s.anime_id == anime_id && s.series_no == series_no && s.season_id == season_id) {
                    return Ok(s.clone());
                }
            }
            // Create new
            let mut series = self.series.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_series = AnimeSeries {
                series_id: *next_id,
                anime_id,
                series_no,
                season_id,
                description,
                aired_date: None,
                end_date: None,
                created_at: now,
                updated_at: now,
            };
            *next_id += 1;
            series.push(new_series.clone());
            Ok(new_series)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::MockAnimeSeriesRepository;

    #[tokio::test]
    async fn test_mock_anime_series_repository_create() {
        let repo = MockAnimeSeriesRepository::new();
        let params = CreateAnimeSeriesParams {
            anime_id: 1,
            series_no: 1,
            season_id: 1,
            description: Some("Test description".to_string()),
            aired_date: None,
            end_date: None,
        };
        let series = repo.create(params).await.unwrap();

        assert_eq!(series.series_id, 1);
        assert_eq!(series.anime_id, 1);
        assert_eq!(series.series_no, 1);

        let ops = repo.get_operations();
        assert!(ops.contains(&"create:anime_id:1".to_string()));
    }

    #[tokio::test]
    async fn test_mock_anime_series_repository_find_by_id() {
        let series = AnimeSeries {
            series_id: 1,
            anime_id: 1,
            series_no: 1,
            season_id: 1,
            description: Some("Test".to_string()),
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeSeriesRepository::with_data(vec![series]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().series_id, 1);

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_mock_anime_series_repository_find_by_anime_id() {
        let series1 = AnimeSeries {
            series_id: 1,
            anime_id: 1,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let series2 = AnimeSeries {
            series_id: 2,
            anime_id: 1,
            series_no: 2,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let series3 = AnimeSeries {
            series_id: 3,
            anime_id: 2,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeSeriesRepository::with_data(vec![series1, series2, series3]);

        let anime1_series = repo.find_by_anime_id(1).await.unwrap();
        assert_eq!(anime1_series.len(), 2);

        let anime2_series = repo.find_by_anime_id(2).await.unwrap();
        assert_eq!(anime2_series.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_anime_series_repository_delete() {
        let series = AnimeSeries {
            series_id: 1,
            anime_id: 1,
            series_no: 1,
            season_id: 1,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };
        let repo = MockAnimeSeriesRepository::with_data(vec![series]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
