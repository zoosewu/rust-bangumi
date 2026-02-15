use async_trait::async_trait;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::{NewTitleParser, TitleParser};
use crate::schema::title_parsers;

#[async_trait]
pub trait TitleParserRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<TitleParser>, RepositoryError>;
    async fn find_all_sorted_by_priority(&self) -> Result<Vec<TitleParser>, RepositoryError>;
    async fn find_enabled_sorted_by_priority(&self) -> Result<Vec<TitleParser>, RepositoryError>;
    async fn create(&self, parser: NewTitleParser) -> Result<TitleParser, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
}

pub struct DieselTitleParserRepository {
    pool: DbPool,
}

impl DieselTitleParserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TitleParserRepository for DieselTitleParserRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<TitleParser>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            title_parsers::table
                .filter(title_parsers::parser_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all_sorted_by_priority(&self) -> Result<Vec<TitleParser>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            title_parsers::table
                .order(title_parsers::priority.desc())
                .load::<TitleParser>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_enabled_sorted_by_priority(&self) -> Result<Vec<TitleParser>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            title_parsers::table
                .filter(title_parsers::is_enabled.eq(true))
                .order(title_parsers::priority.desc())
                .load::<TitleParser>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, parser: NewTitleParser) -> Result<TitleParser, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(title_parsers::table)
                .values(&parser)
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
                diesel::delete(title_parsers::table.filter(title_parsers::parser_id.eq(id)))
                    .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use crate::models::ParserSourceType;
    use chrono::Utc;
    use std::sync::Mutex;

    pub struct MockTitleParserRepository {
        parsers: Mutex<Vec<TitleParser>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockTitleParserRepository {
        pub fn new() -> Self {
            Self {
                parsers: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(parsers: Vec<TitleParser>) -> Self {
            let max_id = parsers.iter().map(|p| p.parser_id).max().unwrap_or(0);
            Self {
                parsers: Mutex::new(parsers),
                next_id: Mutex::new(max_id + 1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockTitleParserRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl TitleParserRepository for MockTitleParserRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<TitleParser>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("find_by_id:{}", id));
            Ok(self
                .parsers
                .lock()
                .unwrap()
                .iter()
                .find(|p| p.parser_id == id)
                .cloned())
        }

        async fn find_all_sorted_by_priority(&self) -> Result<Vec<TitleParser>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push("find_all_sorted_by_priority".to_string());
            let mut parsers = self.parsers.lock().unwrap().clone();
            parsers.sort_by(|a, b| b.priority.cmp(&a.priority));
            Ok(parsers)
        }

        async fn find_enabled_sorted_by_priority(
            &self,
        ) -> Result<Vec<TitleParser>, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push("find_enabled_sorted_by_priority".to_string());
            let mut parsers: Vec<TitleParser> = self
                .parsers
                .lock()
                .unwrap()
                .iter()
                .filter(|p| p.is_enabled)
                .cloned()
                .collect();
            parsers.sort_by(|a, b| b.priority.cmp(&a.priority));
            Ok(parsers)
        }

        async fn create(&self, parser: NewTitleParser) -> Result<TitleParser, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("create:{}", parser.name));
            let mut parsers = self.parsers.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let new_parser = TitleParser {
                parser_id: *next_id,
                name: parser.name,
                description: parser.description,
                priority: parser.priority,
                is_enabled: parser.is_enabled,
                condition_regex: parser.condition_regex,
                parse_regex: parser.parse_regex,
                anime_title_source: parser.anime_title_source,
                anime_title_value: parser.anime_title_value,
                episode_no_source: parser.episode_no_source,
                episode_no_value: parser.episode_no_value,
                series_no_source: parser.series_no_source,
                series_no_value: parser.series_no_value,
                subtitle_group_source: parser.subtitle_group_source,
                subtitle_group_value: parser.subtitle_group_value,
                resolution_source: parser.resolution_source,
                resolution_value: parser.resolution_value,
                season_source: parser.season_source,
                season_value: parser.season_value,
                year_source: parser.year_source,
                year_value: parser.year_value,
                created_at: parser.created_at,
                updated_at: parser.updated_at,
                created_from_type: parser.created_from_type,
                created_from_id: parser.created_from_id,
            };
            *next_id += 1;
            parsers.push(new_parser.clone());
            Ok(new_parser)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations
                .lock()
                .unwrap()
                .push(format!("delete:{}", id));
            let mut parsers = self.parsers.lock().unwrap();
            let original_len = parsers.len();
            parsers.retain(|p| p.parser_id != id);
            Ok(parsers.len() < original_len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockTitleParserRepository;
    use super::*;
    use crate::models::ParserSourceType;
    use chrono::Utc;

    fn create_test_parser(id: i32, name: &str, priority: i32, enabled: bool) -> TitleParser {
        let now = Utc::now().naive_utc();
        TitleParser {
            parser_id: id,
            name: name.to_string(),
            description: None,
            priority,
            is_enabled: enabled,
            condition_regex: ".*".to_string(),
            parse_regex: "(.*)".to_string(),
            anime_title_source: ParserSourceType::Regex,
            anime_title_value: "$1".to_string(),
            episode_no_source: ParserSourceType::Static,
            episode_no_value: "1".to_string(),
            series_no_source: None,
            series_no_value: None,
            subtitle_group_source: None,
            subtitle_group_value: None,
            resolution_source: None,
            resolution_value: None,
            season_source: None,
            season_value: None,
            year_source: None,
            year_value: None,
            created_at: now,
            updated_at: now,
            created_from_type: None,
            created_from_id: None,
        }
    }

    #[tokio::test]
    async fn test_mock_title_parser_repository_find_all_sorted() {
        let parser1 = create_test_parser(1, "Parser A", 10, true);
        let parser2 = create_test_parser(2, "Parser B", 50, true);
        let parser3 = create_test_parser(3, "Parser C", 30, false);
        let repo = MockTitleParserRepository::with_data(vec![parser1, parser2, parser3]);

        let all = repo.find_all_sorted_by_priority().await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "Parser B"); // highest priority first
        assert_eq!(all[1].name, "Parser C");
        assert_eq!(all[2].name, "Parser A");
    }

    #[tokio::test]
    async fn test_mock_title_parser_repository_find_enabled() {
        let parser1 = create_test_parser(1, "Parser A", 10, true);
        let parser2 = create_test_parser(2, "Parser B", 50, true);
        let parser3 = create_test_parser(3, "Parser C", 30, false);
        let repo = MockTitleParserRepository::with_data(vec![parser1, parser2, parser3]);

        let enabled = repo.find_enabled_sorted_by_priority().await.unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].name, "Parser B");
        assert_eq!(enabled[1].name, "Parser A");
    }

    #[tokio::test]
    async fn test_mock_title_parser_repository_delete() {
        let parser = create_test_parser(1, "Test", 10, true);
        let repo = MockTitleParserRepository::with_data(vec![parser]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
