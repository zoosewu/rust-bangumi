use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::SubscriptionConflict;
use crate::schema::{subscription_conflicts, subscriptions};

#[async_trait]
pub trait ConflictRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<SubscriptionConflict>, RepositoryError>;
    async fn find_unresolved(&self) -> Result<Vec<SubscriptionConflict>, RepositoryError>;
    async fn resolve(
        &self,
        id: i32,
        fetcher_id: i32,
        subscription_id: i32,
    ) -> Result<SubscriptionConflict, RepositoryError>;
}

pub struct DieselConflictRepository {
    pool: DbPool,
}

impl DieselConflictRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConflictRepository for DieselConflictRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<SubscriptionConflict>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscription_conflicts::table
                .filter(subscription_conflicts::conflict_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_unresolved(&self) -> Result<Vec<SubscriptionConflict>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscription_conflicts::table
                .filter(subscription_conflicts::resolution_status.eq("unresolved"))
                .load::<SubscriptionConflict>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn resolve(
        &self,
        id: i32,
        fetcher_id: i32,
        subscription_id: i32,
    ) -> Result<SubscriptionConflict, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();

            // Update subscription's fetcher_id
            diesel::update(
                subscriptions::table.filter(subscriptions::subscription_id.eq(subscription_id)),
            )
            .set(subscriptions::fetcher_id.eq(fetcher_id))
            .execute(&mut conn)?;

            // Update conflict as resolved
            let resolution_data = serde_json::json!({
                "resolved_fetcher_id": fetcher_id,
                "resolved_at": now.to_string()
            })
            .to_string();

            diesel::update(
                subscription_conflicts::table.filter(subscription_conflicts::conflict_id.eq(id)),
            )
            .set((
                subscription_conflicts::resolution_status.eq("resolved"),
                subscription_conflicts::resolution_data.eq(resolution_data),
                subscription_conflicts::resolved_at.eq(now),
            ))
            .get_result::<SubscriptionConflict>(&mut conn)
            .map_err(RepositoryError::from)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockConflictRepository {
        conflicts: Mutex<Vec<SubscriptionConflict>>,
        operations: Mutex<Vec<String>>,
    }

    impl MockConflictRepository {
        pub fn new() -> Self {
            Self {
                conflicts: Mutex::new(Vec::new()),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(conflicts: Vec<SubscriptionConflict>) -> Self {
            Self {
                conflicts: Mutex::new(conflicts),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockConflictRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl ConflictRepository for MockConflictRepository {
        async fn find_by_id(
            &self,
            id: i32,
        ) -> Result<Option<SubscriptionConflict>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .conflicts
                .lock()
                .unwrap()
                .iter()
                .find(|c| c.conflict_id == id)
                .cloned())
        }

        async fn find_unresolved(&self) -> Result<Vec<SubscriptionConflict>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push("find_unresolved".to_string());
            Ok(self
                .conflicts
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.resolution_status == "unresolved")
                .cloned()
                .collect())
        }

        async fn resolve(
            &self,
            id: i32,
            fetcher_id: i32,
            _subscription_id: i32,
        ) -> Result<SubscriptionConflict, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("resolve:{}:{}", id, fetcher_id));
            let mut conflicts = self.conflicts.lock().unwrap();
            if let Some(conflict) = conflicts.iter_mut().find(|c| c.conflict_id == id) {
                let now = Utc::now().naive_utc();
                conflict.resolution_status = "resolved".to_string();
                conflict.resolution_data = Some(
                    serde_json::json!({
                        "resolved_fetcher_id": fetcher_id
                    })
                    .to_string(),
                );
                conflict.resolved_at = Some(now);
                return Ok(conflict.clone());
            }
            Err(RepositoryError::NotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockConflictRepository;
    use super::*;

    fn create_test_conflict(id: i32, status: &str) -> SubscriptionConflict {
        let now = Utc::now().naive_utc();
        SubscriptionConflict {
            conflict_id: id,
            subscription_id: 1,
            conflict_type: "multiple_fetchers".to_string(),
            affected_item_id: None,
            conflict_data: r#"{"candidate_fetcher_ids":[1,2]}"#.to_string(),
            resolution_status: status.to_string(),
            resolution_data: None,
            created_at: now,
            resolved_at: None,
        }
    }

    #[tokio::test]
    async fn test_mock_conflict_repository_find_unresolved() {
        let conflict1 = create_test_conflict(1, "unresolved");
        let conflict2 = create_test_conflict(2, "resolved");
        let repo = MockConflictRepository::with_data(vec![conflict1, conflict2]);

        let unresolved = repo.find_unresolved().await.unwrap();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].conflict_id, 1);
    }

    #[tokio::test]
    async fn test_mock_conflict_repository_resolve() {
        let conflict = create_test_conflict(1, "unresolved");
        let repo = MockConflictRepository::with_data(vec![conflict]);

        let resolved = repo.resolve(1, 5, 1).await.unwrap();
        assert_eq!(resolved.resolution_status, "resolved");
        assert!(resolved.resolved_at.is_some());
    }

    #[tokio::test]
    async fn test_mock_conflict_repository_find_by_id() {
        let conflict = create_test_conflict(1, "unresolved");
        let repo = MockConflictRepository::with_data(vec![conflict]);

        let found = repo.find_by_id(1).await.unwrap();
        assert!(found.is_some());

        let not_found = repo.find_by_id(999).await.unwrap();
        assert!(not_found.is_none());
    }
}
