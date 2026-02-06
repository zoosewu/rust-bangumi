use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{NewSeason, Season};
use crate::schema::seasons;

#[async_trait]
pub trait SeasonRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Season>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<Season>, RepositoryError>;
    async fn create(&self, year: i32, season: String) -> Result<Season, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    async fn find_or_create(&self, year: i32, season: String) -> Result<Season, RepositoryError>;
}

pub struct DieselSeasonRepository {
    pool: DbPool,
}

impl DieselSeasonRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SeasonRepository for DieselSeasonRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<Season>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            seasons::table
                .filter(seasons::season_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<Season>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            seasons::table
                .load::<Season>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, year: i32, season: String) -> Result<Season, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            let new_season = NewSeason {
                year,
                season,
                created_at: now,
            };
            diesel::insert_into(seasons::table)
                .values(&new_season)
                .get_result(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let deleted = diesel::delete(seasons::table.filter(seasons::season_id.eq(id)))
                .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }

    async fn find_or_create(&self, year: i32, season: String) -> Result<Season, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            // Try to find existing
            match seasons::table
                .filter(seasons::year.eq(year))
                .filter(seasons::season.eq(&season))
                .first::<Season>(&mut conn)
            {
                Ok(s) => Ok(s),
                Err(diesel::NotFound) => {
                    // Create new
                    let now = Utc::now().naive_utc();
                    let new_season = NewSeason {
                        year,
                        season,
                        created_at: now,
                    };
                    diesel::insert_into(seasons::table)
                        .values(&new_season)
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

    pub struct MockSeasonRepository {
        seasons: Mutex<Vec<Season>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockSeasonRepository {
        pub fn new() -> Self {
            Self {
                seasons: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(seasons: Vec<Season>) -> Self {
            let max_id = seasons.iter().map(|s| s.season_id).max().unwrap_or(0);
            Self {
                seasons: Mutex::new(seasons),
                next_id: Mutex::new(max_id + 1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockSeasonRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl SeasonRepository for MockSeasonRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<Season>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .seasons
                .lock()
                .unwrap()
                .iter()
                .find(|s| s.season_id == id)
                .cloned())
        }

        async fn find_all(&self) -> Result<Vec<Season>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.seasons.lock().unwrap().clone())
        }

        async fn create(&self, year: i32, season: String) -> Result<Season, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("create:{}:{}", year, season));
            let mut seasons = self.seasons.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_season = Season {
                season_id: *next_id,
                year,
                season,
                created_at: now,
            };
            *next_id += 1;
            seasons.push(new_season.clone());
            Ok(new_season)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("delete:{}", id));
            let mut seasons = self.seasons.lock().unwrap();
            let original_len = seasons.len();
            seasons.retain(|s| s.season_id != id);
            Ok(seasons.len() < original_len)
        }

        async fn find_or_create(
            &self,
            year: i32,
            season: String,
        ) -> Result<Season, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_or_create:{}:{}", year, season));
            // Try to find existing
            {
                let seasons = self.seasons.lock().unwrap();
                if let Some(s) = seasons
                    .iter()
                    .find(|s| s.year == year && s.season == season)
                {
                    return Ok(s.clone());
                }
            }
            // Create new
            let mut seasons = self.seasons.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_season = Season {
                season_id: *next_id,
                year,
                season,
                created_at: now,
            };
            *next_id += 1;
            seasons.push(new_season.clone());
            Ok(new_season)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockSeasonRepository;
    use super::*;

    #[tokio::test]
    async fn test_mock_season_repository_create() {
        let repo = MockSeasonRepository::new();
        let season = repo.create(2024, "Winter".to_string()).await.unwrap();

        assert_eq!(season.season_id, 1);
        assert_eq!(season.year, 2024);
        assert_eq!(season.season, "Winter");

        let ops = repo.get_operations();
        assert!(ops.contains(&"create:2024:Winter".to_string()));
    }

    #[tokio::test]
    async fn test_mock_season_repository_find_by_id() {
        let season = Season {
            season_id: 1,
            year: 2024,
            season: "Spring".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSeasonRepository::with_data(vec![season]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().season, "Spring");

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_mock_season_repository_find_all() {
        let season1 = Season {
            season_id: 1,
            year: 2024,
            season: "Winter".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let season2 = Season {
            season_id: 2,
            year: 2024,
            season: "Spring".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSeasonRepository::with_data(vec![season1, season2]);

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_season_repository_delete() {
        let season = Season {
            season_id: 1,
            year: 2024,
            season: "Winter".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSeasonRepository::with_data(vec![season]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
