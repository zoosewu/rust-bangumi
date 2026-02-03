use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::{NewSubscription, Subscription};
use crate::schema::subscriptions;
use super::RepositoryError;

#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Subscription>, RepositoryError>;
    async fn find_by_source_url(&self, source_url: &str) -> Result<Option<Subscription>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<Subscription>, RepositoryError>;
    async fn find_active(&self) -> Result<Vec<Subscription>, RepositoryError>;
    async fn find_by_fetcher_id(&self, fetcher_id: i32) -> Result<Vec<Subscription>, RepositoryError>;
    async fn find_pending_assignment(&self) -> Result<Vec<Subscription>, RepositoryError>;
    async fn create(&self, new_subscription: NewSubscription) -> Result<Subscription, RepositoryError>;
    async fn update(&self, subscription: &Subscription) -> Result<Subscription, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
    async fn delete_by_source_url(&self, source_url: &str) -> Result<bool, RepositoryError>;
    async fn update_assignment_status(
        &self,
        id: i32,
        status: &str,
        assigned_at: Option<NaiveDateTime>,
    ) -> Result<Subscription, RepositoryError>;
    async fn update_fetch_times(
        &self,
        id: i32,
        last_fetched_at: NaiveDateTime,
        next_fetch_at: NaiveDateTime,
    ) -> Result<Subscription, RepositoryError>;
}

pub struct DieselSubscriptionRepository {
    pool: DbPool,
}

impl DieselSubscriptionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubscriptionRepository for DieselSubscriptionRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<Subscription>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscriptions::table
                .find(id)
                .first::<Subscription>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_source_url(&self, source_url: &str) -> Result<Option<Subscription>, RepositoryError> {
        let pool = self.pool.clone();
        let source_url = source_url.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscriptions::table
                .filter(subscriptions::source_url.eq(&source_url))
                .first::<Subscription>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<Subscription>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscriptions::table
                .load::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_active(&self) -> Result<Vec<Subscription>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscriptions::table
                .filter(subscriptions::is_active.eq(true))
                .load::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_fetcher_id(&self, fetcher_id: i32) -> Result<Vec<Subscription>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscriptions::table
                .filter(subscriptions::fetcher_id.eq(fetcher_id))
                .load::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_pending_assignment(&self) -> Result<Vec<Subscription>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            subscriptions::table
                .filter(subscriptions::assignment_status.eq("pending"))
                .filter(subscriptions::is_active.eq(true))
                .load::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, new_subscription: NewSubscription) -> Result<Subscription, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(subscriptions::table)
                .values(&new_subscription)
                .get_result::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update(&self, subscription: &Subscription) -> Result<Subscription, RepositoryError> {
        let pool = self.pool.clone();
        let sub = subscription.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(subscriptions::table.find(sub.subscription_id))
                .set((
                    subscriptions::fetcher_id.eq(sub.fetcher_id),
                    subscriptions::source_url.eq(&sub.source_url),
                    subscriptions::name.eq(&sub.name),
                    subscriptions::description.eq(&sub.description),
                    subscriptions::fetch_interval_minutes.eq(sub.fetch_interval_minutes),
                    subscriptions::is_active.eq(sub.is_active),
                    subscriptions::config.eq(&sub.config),
                    subscriptions::source_type.eq(&sub.source_type),
                    subscriptions::auto_selected.eq(sub.auto_selected),
                    subscriptions::updated_at.eq(Utc::now().naive_utc()),
                ))
                .get_result::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let rows_deleted = diesel::delete(subscriptions::table.find(id))
                .execute(&mut conn)?;
            Ok(rows_deleted > 0)
        })
        .await?
    }

    async fn delete_by_source_url(&self, source_url: &str) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        let source_url = source_url.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let rows_deleted = diesel::delete(
                subscriptions::table.filter(subscriptions::source_url.eq(&source_url))
            ).execute(&mut conn)?;
            Ok(rows_deleted > 0)
        })
        .await?
    }

    async fn update_assignment_status(
        &self,
        id: i32,
        status: &str,
        assigned_at: Option<NaiveDateTime>,
    ) -> Result<Subscription, RepositoryError> {
        let pool = self.pool.clone();
        let status = status.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(subscriptions::table.find(id))
                .set((
                    subscriptions::assignment_status.eq(&status),
                    subscriptions::assigned_at.eq(assigned_at),
                    subscriptions::updated_at.eq(Utc::now().naive_utc()),
                ))
                .get_result::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update_fetch_times(
        &self,
        id: i32,
        last_fetched_at: NaiveDateTime,
        next_fetch_at: NaiveDateTime,
    ) -> Result<Subscription, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(subscriptions::table.find(id))
                .set((
                    subscriptions::last_fetched_at.eq(Some(last_fetched_at)),
                    subscriptions::next_fetch_at.eq(Some(next_fetch_at)),
                    subscriptions::updated_at.eq(Utc::now().naive_utc()),
                ))
                .get_result::<Subscription>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockSubscriptionRepository {
        pub subscriptions: Mutex<Vec<Subscription>>,
        pub operations: Mutex<Vec<String>>,
    }

    impl MockSubscriptionRepository {
        pub fn new() -> Self {
            Self {
                subscriptions: Mutex::new(Vec::new()),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(subscriptions: Vec<Subscription>) -> Self {
            Self {
                subscriptions: Mutex::new(subscriptions),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockSubscriptionRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl SubscriptionRepository for MockSubscriptionRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<Subscription>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_id:{}", id));
            Ok(self.subscriptions.lock().unwrap()
                .iter()
                .find(|s| s.subscription_id == id)
                .cloned())
        }

        async fn find_by_source_url(&self, source_url: &str) -> Result<Option<Subscription>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_source_url:{}", source_url));
            Ok(self.subscriptions.lock().unwrap()
                .iter()
                .find(|s| s.source_url == source_url)
                .cloned())
        }

        async fn find_all(&self) -> Result<Vec<Subscription>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.subscriptions.lock().unwrap().clone())
        }

        async fn find_active(&self) -> Result<Vec<Subscription>, RepositoryError> {
            self.operations.lock().unwrap().push("find_active".to_string());
            Ok(self.subscriptions.lock().unwrap()
                .iter()
                .filter(|s| s.is_active)
                .cloned()
                .collect())
        }

        async fn find_by_fetcher_id(&self, fetcher_id: i32) -> Result<Vec<Subscription>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_fetcher_id:{}", fetcher_id));
            Ok(self.subscriptions.lock().unwrap()
                .iter()
                .filter(|s| s.fetcher_id == fetcher_id)
                .cloned()
                .collect())
        }

        async fn find_pending_assignment(&self) -> Result<Vec<Subscription>, RepositoryError> {
            self.operations.lock().unwrap().push("find_pending_assignment".to_string());
            Ok(self.subscriptions.lock().unwrap()
                .iter()
                .filter(|s| s.assignment_status == "pending" && s.is_active)
                .cloned()
                .collect())
        }

        async fn create(&self, new_subscription: NewSubscription) -> Result<Subscription, RepositoryError> {
            self.operations.lock().unwrap().push("create".to_string());
            let mut subs = self.subscriptions.lock().unwrap();
            let id = subs.len() as i32 + 1;
            let subscription = Subscription {
                subscription_id: id,
                fetcher_id: new_subscription.fetcher_id,
                source_url: new_subscription.source_url,
                name: new_subscription.name,
                description: new_subscription.description,
                last_fetched_at: new_subscription.last_fetched_at,
                next_fetch_at: new_subscription.next_fetch_at,
                fetch_interval_minutes: new_subscription.fetch_interval_minutes,
                is_active: new_subscription.is_active,
                config: new_subscription.config,
                created_at: new_subscription.created_at,
                updated_at: new_subscription.updated_at,
                source_type: new_subscription.source_type,
                assignment_status: new_subscription.assignment_status,
                assigned_at: new_subscription.assigned_at,
                auto_selected: new_subscription.auto_selected,
            };
            subs.push(subscription.clone());
            Ok(subscription)
        }

        async fn update(&self, subscription: &Subscription) -> Result<Subscription, RepositoryError> {
            self.operations.lock().unwrap().push(format!("update:{}", subscription.subscription_id));
            let mut subs = self.subscriptions.lock().unwrap();
            if let Some(pos) = subs.iter().position(|s| s.subscription_id == subscription.subscription_id) {
                subs[pos] = subscription.clone();
                Ok(subscription.clone())
            } else {
                Err(RepositoryError::NotFound)
            }
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations.lock().unwrap().push(format!("delete:{}", id));
            let mut subs = self.subscriptions.lock().unwrap();
            let len_before = subs.len();
            subs.retain(|s| s.subscription_id != id);
            Ok(subs.len() < len_before)
        }

        async fn delete_by_source_url(&self, source_url: &str) -> Result<bool, RepositoryError> {
            self.operations.lock().unwrap().push(format!("delete_by_source_url:{}", source_url));
            let mut subs = self.subscriptions.lock().unwrap();
            let len_before = subs.len();
            subs.retain(|s| s.source_url != source_url);
            Ok(subs.len() < len_before)
        }

        async fn update_assignment_status(
            &self,
            id: i32,
            status: &str,
            assigned_at: Option<NaiveDateTime>,
        ) -> Result<Subscription, RepositoryError> {
            self.operations.lock().unwrap().push(format!("update_assignment_status:{}:{}", id, status));
            let mut subs = self.subscriptions.lock().unwrap();
            if let Some(pos) = subs.iter().position(|s| s.subscription_id == id) {
                subs[pos].assignment_status = status.to_string();
                subs[pos].assigned_at = assigned_at;
                subs[pos].updated_at = Utc::now().naive_utc();
                Ok(subs[pos].clone())
            } else {
                Err(RepositoryError::NotFound)
            }
        }

        async fn update_fetch_times(
            &self,
            id: i32,
            last_fetched_at: NaiveDateTime,
            next_fetch_at: NaiveDateTime,
        ) -> Result<Subscription, RepositoryError> {
            self.operations.lock().unwrap().push(format!("update_fetch_times:{}", id));
            let mut subs = self.subscriptions.lock().unwrap();
            if let Some(pos) = subs.iter().position(|s| s.subscription_id == id) {
                subs[pos].last_fetched_at = Some(last_fetched_at);
                subs[pos].next_fetch_at = Some(next_fetch_at);
                subs[pos].updated_at = Utc::now().naive_utc();
                Ok(subs[pos].clone())
            } else {
                Err(RepositoryError::NotFound)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn create_test_subscription(id: i32, is_active: bool, status: &str) -> Subscription {
            let now = Utc::now().naive_utc();
            Subscription {
                subscription_id: id,
                fetcher_id: 1,
                source_url: format!("http://example.com/feed{}", id),
                name: Some(format!("Test Subscription {}", id)),
                description: None,
                last_fetched_at: None,
                next_fetch_at: None,
                fetch_interval_minutes: 60,
                is_active,
                config: None,
                created_at: now,
                updated_at: now,
                source_type: "rss".to_string(),
                assignment_status: status.to_string(),
                assigned_at: None,
                auto_selected: false,
            }
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_find_by_id() {
            let sub = create_test_subscription(1, true, "assigned");
            let repo = MockSubscriptionRepository::with_data(vec![sub]);

            let found = repo.find_by_id(1).await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().subscription_id, 1);

            let not_found = repo.find_by_id(999).await.unwrap();
            assert!(not_found.is_none());

            let ops = repo.get_operations();
            assert_eq!(ops, vec!["find_by_id:1", "find_by_id:999"]);
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_find_active() {
            let sub1 = create_test_subscription(1, true, "assigned");
            let sub2 = create_test_subscription(2, false, "assigned");
            let sub3 = create_test_subscription(3, true, "pending");
            let repo = MockSubscriptionRepository::with_data(vec![sub1, sub2, sub3]);

            let active = repo.find_active().await.unwrap();
            assert_eq!(active.len(), 2);
            assert!(active.iter().all(|s| s.is_active));
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_find_pending_assignment() {
            let sub1 = create_test_subscription(1, true, "pending");
            let sub2 = create_test_subscription(2, true, "assigned");
            let sub3 = create_test_subscription(3, false, "pending"); // inactive
            let repo = MockSubscriptionRepository::with_data(vec![sub1, sub2, sub3]);

            let pending = repo.find_pending_assignment().await.unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].subscription_id, 1);
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_create() {
            let repo = MockSubscriptionRepository::new();
            let now = Utc::now().naive_utc();

            let new_sub = NewSubscription {
                fetcher_id: 1,
                source_url: "http://example.com/feed".to_string(),
                name: Some("New Sub".to_string()),
                description: None,
                last_fetched_at: None,
                next_fetch_at: None,
                fetch_interval_minutes: 30,
                is_active: true,
                config: None,
                created_at: now,
                updated_at: now,
                source_type: "rss".to_string(),
                assignment_status: "pending".to_string(),
                assigned_at: None,
                auto_selected: false,
            };

            let created = repo.create(new_sub).await.unwrap();
            assert_eq!(created.subscription_id, 1);
            assert_eq!(created.source_url, "http://example.com/feed");

            let ops = repo.get_operations();
            assert_eq!(ops, vec!["create"]);
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_update() {
            let mut sub = create_test_subscription(1, true, "pending");
            let repo = MockSubscriptionRepository::with_data(vec![sub.clone()]);

            sub.assignment_status = "assigned".to_string();
            let updated = repo.update(&sub).await.unwrap();
            assert_eq!(updated.assignment_status, "assigned");

            // Test update non-existent
            sub.subscription_id = 999;
            let result = repo.update(&sub).await;
            assert!(matches!(result, Err(RepositoryError::NotFound)));
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_delete() {
            let sub = create_test_subscription(1, true, "assigned");
            let repo = MockSubscriptionRepository::with_data(vec![sub]);

            let deleted = repo.delete(1).await.unwrap();
            assert!(deleted);

            let not_deleted = repo.delete(999).await.unwrap();
            assert!(!not_deleted);

            // Verify it's actually deleted
            let found = repo.find_by_id(1).await.unwrap();
            assert!(found.is_none());
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_update_assignment_status() {
            let sub = create_test_subscription(1, true, "pending");
            let repo = MockSubscriptionRepository::with_data(vec![sub]);
            let now = Utc::now().naive_utc();

            let updated = repo.update_assignment_status(1, "assigned", Some(now)).await.unwrap();
            assert_eq!(updated.assignment_status, "assigned");
            assert!(updated.assigned_at.is_some());

            let result = repo.update_assignment_status(999, "assigned", None).await;
            assert!(matches!(result, Err(RepositoryError::NotFound)));
        }

        #[tokio::test]
        async fn test_mock_subscription_repository_update_fetch_times() {
            let sub = create_test_subscription(1, true, "assigned");
            let repo = MockSubscriptionRepository::with_data(vec![sub]);
            let now = Utc::now().naive_utc();
            let next = now + chrono::Duration::hours(1);

            let updated = repo.update_fetch_times(1, now, next).await.unwrap();
            assert_eq!(updated.last_fetched_at, Some(now));
            assert_eq!(updated.next_fetch_at, Some(next));

            let result = repo.update_fetch_times(999, now, next).await;
            assert!(matches!(result, Err(RepositoryError::NotFound)));
        }
    }
}
