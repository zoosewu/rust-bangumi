use async_trait::async_trait;
use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};

use crate::db::DbPool;
use crate::models::RawAnimeItem;
use crate::schema::raw_anime_items;
use super::RepositoryError;

#[derive(Debug, Clone)]
pub struct RawItemFilter {
    pub status: Option<String>,
    pub subscription_id: Option<i32>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for RawItemFilter {
    fn default() -> Self {
        Self {
            status: None,
            subscription_id: None,
            limit: 100,
            offset: 0,
        }
    }
}

#[async_trait]
pub trait RawItemRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<RawAnimeItem>, RepositoryError>;
    async fn find_with_filters(&self, filter: RawItemFilter) -> Result<Vec<RawAnimeItem>, RepositoryError>;
    async fn save(&self, title: &str, description: Option<&str>, download_url: &str, pub_date: Option<NaiveDateTime>, subscription_id: i32) -> Result<RawAnimeItem, RepositoryError>;
    async fn update_status(&self, id: i32, status: &str, parser_id: Option<i32>, error_message: Option<&str>) -> Result<(), RepositoryError>;
}

pub struct DieselRawItemRepository {
    pool: DbPool,
}

impl DieselRawItemRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RawItemRepository for DieselRawItemRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<RawAnimeItem>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            raw_anime_items::table
                .filter(raw_anime_items::item_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_with_filters(&self, filter: RawItemFilter) -> Result<Vec<RawAnimeItem>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let mut query = raw_anime_items::table.into_boxed();

            if let Some(status) = &filter.status {
                query = query.filter(raw_anime_items::status.eq(status));
            }

            if let Some(sub_id) = filter.subscription_id {
                query = query.filter(raw_anime_items::subscription_id.eq(sub_id));
            }

            query
                .order(raw_anime_items::created_at.desc())
                .limit(filter.limit.min(1000))
                .offset(filter.offset)
                .load::<RawAnimeItem>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn save(&self, title: &str, description: Option<&str>, download_url: &str, pub_date: Option<NaiveDateTime>, subscription_id: i32) -> Result<RawAnimeItem, RepositoryError> {
        let pool = self.pool.clone();
        let title = title.to_string();
        let description = description.map(|s| s.to_string());
        let download_url = download_url.to_string();

        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();

            diesel::insert_into(raw_anime_items::table)
                .values((
                    raw_anime_items::title.eq(&title),
                    raw_anime_items::description.eq(&description),
                    raw_anime_items::download_url.eq(&download_url),
                    raw_anime_items::pub_date.eq(pub_date),
                    raw_anime_items::subscription_id.eq(subscription_id),
                    raw_anime_items::status.eq("pending"),
                    raw_anime_items::created_at.eq(now),
                ))
                .get_result(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn update_status(&self, id: i32, status: &str, parser_id: Option<i32>, error_message: Option<&str>) -> Result<(), RepositoryError> {
        let pool = self.pool.clone();
        let status = status.to_string();
        let error_message = error_message.map(|s| s.to_string());

        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();

            diesel::update(raw_anime_items::table.filter(raw_anime_items::item_id.eq(id)))
                .set((
                    raw_anime_items::status.eq(&status),
                    raw_anime_items::parser_id.eq(parser_id),
                    raw_anime_items::error_message.eq(&error_message),
                    raw_anime_items::parsed_at.eq(Some(now)),
                ))
                .execute(&mut conn)?;
            Ok(())
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockRawItemRepository {
        items: Mutex<Vec<RawAnimeItem>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockRawItemRepository {
        pub fn new() -> Self {
            Self {
                items: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(items: Vec<RawAnimeItem>) -> Self {
            let max_id = items.iter().map(|i| i.item_id).max().unwrap_or(0);
            Self {
                items: Mutex::new(items),
                next_id: Mutex::new(max_id + 1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockRawItemRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl RawItemRepository for MockRawItemRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<RawAnimeItem>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_id:{}", id));
            Ok(self.items.lock().unwrap().iter().find(|i| i.item_id == id).cloned())
        }

        async fn find_with_filters(&self, filter: RawItemFilter) -> Result<Vec<RawAnimeItem>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_with_filters:status={:?}", filter.status));
            let items = self.items.lock().unwrap();
            let mut result: Vec<RawAnimeItem> = items.iter()
                .filter(|i| {
                    let status_match = filter.status.as_ref().map_or(true, |s| &i.status == s);
                    let sub_match = filter.subscription_id.map_or(true, |id| i.subscription_id == id);
                    status_match && sub_match
                })
                .cloned()
                .collect();
            result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let start = filter.offset as usize;
            let end = (filter.offset + filter.limit) as usize;
            Ok(result.into_iter().skip(start).take(end - start).collect())
        }

        async fn save(&self, title: &str, description: Option<&str>, download_url: &str, pub_date: Option<NaiveDateTime>, subscription_id: i32) -> Result<RawAnimeItem, RepositoryError> {
            self.operations.lock().unwrap().push(format!("save:{}", title));
            let mut items = self.items.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let now = Utc::now().naive_utc();
            let new_item = RawAnimeItem {
                item_id: *next_id,
                title: title.to_string(),
                description: description.map(|s| s.to_string()),
                download_url: download_url.to_string(),
                pub_date,
                subscription_id,
                status: "pending".to_string(),
                parser_id: None,
                error_message: None,
                parsed_at: None,
                created_at: now,
            };
            *next_id += 1;
            items.push(new_item.clone());
            Ok(new_item)
        }

        async fn update_status(&self, id: i32, status: &str, parser_id: Option<i32>, error_message: Option<&str>) -> Result<(), RepositoryError> {
            self.operations.lock().unwrap().push(format!("update_status:{}:{}", id, status));
            let mut items = self.items.lock().unwrap();
            if let Some(item) = items.iter_mut().find(|i| i.item_id == id) {
                item.status = status.to_string();
                item.parser_id = parser_id;
                item.error_message = error_message.map(|s| s.to_string());
                item.parsed_at = Some(Utc::now().naive_utc());
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::MockRawItemRepository;

    #[tokio::test]
    async fn test_mock_raw_item_repository_save() {
        let repo = MockRawItemRepository::new();
        let item = repo.save("Test Title", Some("Description"), "http://example.com", None, 1).await.unwrap();
        assert_eq!(item.item_id, 1);
        assert_eq!(item.title, "Test Title");
        assert_eq!(item.status, "pending");
    }

    #[tokio::test]
    async fn test_mock_raw_item_repository_find_with_filters() {
        let now = Utc::now().naive_utc();
        let item1 = RawAnimeItem {
            item_id: 1,
            title: "Title 1".to_string(),
            description: None,
            download_url: "http://1.com".to_string(),
            pub_date: None,
            subscription_id: 1,
            status: "pending".to_string(),
            parser_id: None,
            error_message: None,
            parsed_at: None,
            created_at: now,
        };
        let item2 = RawAnimeItem {
            item_id: 2,
            title: "Title 2".to_string(),
            description: None,
            download_url: "http://2.com".to_string(),
            pub_date: None,
            subscription_id: 1,
            status: "parsed".to_string(),
            parser_id: Some(1),
            error_message: None,
            parsed_at: Some(now),
            created_at: now,
        };
        let repo = MockRawItemRepository::with_data(vec![item1, item2]);

        let filter = RawItemFilter {
            status: Some("pending".to_string()),
            ..Default::default()
        };
        let pending = repo.find_with_filters(filter).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].title, "Title 1");
    }

    #[tokio::test]
    async fn test_mock_raw_item_repository_update_status() {
        let now = Utc::now().naive_utc();
        let item = RawAnimeItem {
            item_id: 1,
            title: "Test".to_string(),
            description: None,
            download_url: "http://test.com".to_string(),
            pub_date: None,
            subscription_id: 1,
            status: "pending".to_string(),
            parser_id: None,
            error_message: None,
            parsed_at: None,
            created_at: now,
        };
        let repo = MockRawItemRepository::with_data(vec![item]);

        repo.update_status(1, "parsed", Some(5), None).await.unwrap();

        let updated = repo.find_by_id(1).await.unwrap().unwrap();
        assert_eq!(updated.status, "parsed");
        assert_eq!(updated.parser_id, Some(5));
    }
}
