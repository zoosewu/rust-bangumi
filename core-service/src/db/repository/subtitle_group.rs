use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{NewSubtitleGroup, SubtitleGroup};
use crate::schema::subtitle_groups;

#[async_trait]
pub trait SubtitleGroupRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<SubtitleGroup>, RepositoryError>;
    async fn find_by_name(&self, name: &str) -> Result<Option<SubtitleGroup>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<SubtitleGroup>, RepositoryError>;
    async fn create(&self, group_name: String) -> Result<SubtitleGroup, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    async fn find_or_create(&self, group_name: String) -> Result<SubtitleGroup, RepositoryError>;
}

pub struct DieselSubtitleGroupRepository {
    pool: DbPool,
}

impl DieselSubtitleGroupRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubtitleGroupRepository for DieselSubtitleGroupRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<SubtitleGroup>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subtitle_groups::table
                .filter(subtitle_groups::group_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<SubtitleGroup>, RepositoryError> {
        let pool = self.pool.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subtitle_groups::table
                .filter(subtitle_groups::group_name.eq(&name))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<SubtitleGroup>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subtitle_groups::table
                .load::<SubtitleGroup>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, group_name: String) -> Result<SubtitleGroup, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            let new_group = NewSubtitleGroup {
                group_name,
                created_at: now,
            };
            diesel::insert_into(subtitle_groups::table)
                .values(&new_group)
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
                diesel::delete(subtitle_groups::table.filter(subtitle_groups::group_id.eq(id)))
                    .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }

    async fn find_or_create(&self, group_name: String) -> Result<SubtitleGroup, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            // Try to find existing
            match subtitle_groups::table
                .filter(subtitle_groups::group_name.eq(&group_name))
                .first::<SubtitleGroup>(&mut conn)
            {
                Ok(g) => Ok(g),
                Err(diesel::NotFound) => {
                    // Create new
                    let now = Utc::now().naive_utc();
                    let new_group = NewSubtitleGroup {
                        group_name,
                        created_at: now,
                    };
                    diesel::insert_into(subtitle_groups::table)
                        .values(&new_group)
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

    pub struct MockSubtitleGroupRepository {
        groups: Mutex<Vec<SubtitleGroup>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockSubtitleGroupRepository {
        pub fn new() -> Self {
            Self {
                groups: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(groups: Vec<SubtitleGroup>) -> Self {
            let max_id = groups.iter().map(|g| g.group_id).max().unwrap_or(0);
            Self {
                groups: Mutex::new(groups),
                next_id: Mutex::new(max_id + 1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockSubtitleGroupRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl SubtitleGroupRepository for MockSubtitleGroupRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<SubtitleGroup>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .groups
                .lock()
                .unwrap()
                .iter()
                .find(|g| g.group_id == id)
                .cloned())
        }

        async fn find_by_name(&self, name: &str) -> Result<Option<SubtitleGroup>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_name:{}", name));
            Ok(self
                .groups
                .lock()
                .unwrap()
                .iter()
                .find(|g| g.group_name == name)
                .cloned())
        }

        async fn find_all(&self) -> Result<Vec<SubtitleGroup>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.groups.lock().unwrap().clone())
        }

        async fn create(&self, group_name: String) -> Result<SubtitleGroup, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("create:{}", group_name));
            let mut groups = self.groups.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_group = SubtitleGroup {
                group_id: *next_id,
                group_name,
                created_at: now,
            };
            *next_id += 1;
            groups.push(new_group.clone());
            Ok(new_group)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("delete:{}", id));
            let mut groups = self.groups.lock().unwrap();
            let original_len = groups.len();
            groups.retain(|g| g.group_id != id);
            Ok(groups.len() < original_len)
        }

        async fn find_or_create(
            &self,
            group_name: String,
        ) -> Result<SubtitleGroup, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_or_create:{}", group_name));
            // Try to find existing
            {
                let groups = self.groups.lock().unwrap();
                if let Some(g) = groups.iter().find(|g| g.group_name == group_name) {
                    return Ok(g.clone());
                }
            }
            // Create new
            let mut groups = self.groups.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_group = SubtitleGroup {
                group_id: *next_id,
                group_name,
                created_at: now,
            };
            *next_id += 1;
            groups.push(new_group.clone());
            Ok(new_group)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockSubtitleGroupRepository;
    use super::*;

    #[tokio::test]
    async fn test_mock_subtitle_group_repository_create() {
        let repo = MockSubtitleGroupRepository::new();
        let group = repo.create("Test Group".to_string()).await.unwrap();

        assert_eq!(group.group_id, 1);
        assert_eq!(group.group_name, "Test Group");

        let ops = repo.get_operations();
        assert!(ops.contains(&"create:Test Group".to_string()));
    }

    #[tokio::test]
    async fn test_mock_subtitle_group_repository_find_by_id() {
        let group = SubtitleGroup {
            group_id: 1,
            group_name: "Fansub Team".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSubtitleGroupRepository::with_data(vec![group]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().group_name, "Fansub Team");

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_mock_subtitle_group_repository_find_all() {
        let group1 = SubtitleGroup {
            group_id: 1,
            group_name: "Group A".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let group2 = SubtitleGroup {
            group_id: 2,
            group_name: "Group B".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSubtitleGroupRepository::with_data(vec![group1, group2]);

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_subtitle_group_repository_delete() {
        let group = SubtitleGroup {
            group_id: 1,
            group_name: "To Delete".to_string(),
            created_at: Utc::now().naive_utc(),
        };
        let repo = MockSubtitleGroupRepository::with_data(vec![group]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
