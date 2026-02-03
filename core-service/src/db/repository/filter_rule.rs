use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::{FilterRule, NewFilterRule, FilterTargetType};
use crate::schema::filter_rules;
use super::RepositoryError;

#[async_trait]
pub trait FilterRuleRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<FilterRule>, RepositoryError>;
    async fn find_by_target(&self, target_type: FilterTargetType, target_id: Option<i32>) -> Result<Vec<FilterRule>, RepositoryError>;
    async fn create(&self, rule: NewFilterRule) -> Result<FilterRule, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
}

pub struct DieselFilterRuleRepository {
    pool: DbPool,
}

impl DieselFilterRuleRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FilterRuleRepository for DieselFilterRuleRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<FilterRule>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            filter_rules::table
                .filter(filter_rules::rule_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_target(&self, target_type: FilterTargetType, target_id: Option<i32>) -> Result<Vec<FilterRule>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            if target_id.is_some() {
                filter_rules::table
                    .filter(filter_rules::target_type.eq(target_type))
                    .filter(filter_rules::target_id.eq(target_id))
                    .order(filter_rules::rule_order.asc())
                    .load::<FilterRule>(&mut conn)
                    .map_err(RepositoryError::from)
            } else {
                filter_rules::table
                    .filter(filter_rules::target_type.eq(target_type))
                    .filter(filter_rules::target_id.is_null())
                    .order(filter_rules::rule_order.asc())
                    .load::<FilterRule>(&mut conn)
                    .map_err(RepositoryError::from)
            }
        })
        .await?
    }

    async fn create(&self, rule: NewFilterRule) -> Result<FilterRule, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(filter_rules::table)
                .values(&rule)
                .get_result(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let deleted = diesel::delete(filter_rules::table.filter(filter_rules::rule_id.eq(id)))
                .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockFilterRuleRepository {
        rules: Mutex<Vec<FilterRule>>,
        next_id: Mutex<i32>,
        operations: Mutex<Vec<String>>,
    }

    impl MockFilterRuleRepository {
        pub fn new() -> Self {
            Self {
                rules: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(rules: Vec<FilterRule>) -> Self {
            let max_id = rules.iter().map(|r| r.rule_id).max().unwrap_or(0);
            Self {
                rules: Mutex::new(rules),
                next_id: Mutex::new(max_id + 1),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockFilterRuleRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl FilterRuleRepository for MockFilterRuleRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<FilterRule>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_id:{}", id));
            Ok(self.rules.lock().unwrap().iter().find(|r| r.rule_id == id).cloned())
        }

        async fn find_by_target(&self, target_type: FilterTargetType, target_id: Option<i32>) -> Result<Vec<FilterRule>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_target:{:?}:{:?}", target_type, target_id));
            let rules = self.rules.lock().unwrap();
            let mut result: Vec<FilterRule> = rules.iter()
                .filter(|r| r.target_type == target_type && r.target_id == target_id)
                .cloned()
                .collect();
            result.sort_by_key(|r| r.rule_order);
            Ok(result)
        }

        async fn create(&self, rule: NewFilterRule) -> Result<FilterRule, RepositoryError> {
            self.operations.lock().unwrap().push(format!("create:{:?}", rule.target_type));
            let mut rules = self.rules.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let new_rule = FilterRule {
                rule_id: *next_id,
                target_type: rule.target_type,
                target_id: rule.target_id,
                rule_order: rule.rule_order,
                is_positive: rule.is_positive,
                regex_pattern: rule.regex_pattern,
                created_at: rule.created_at,
                updated_at: rule.updated_at,
            };
            *next_id += 1;
            rules.push(new_rule.clone());
            Ok(new_rule)
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations.lock().unwrap().push(format!("delete:{}", id));
            let mut rules = self.rules.lock().unwrap();
            let original_len = rules.len();
            rules.retain(|r| r.rule_id != id);
            Ok(rules.len() < original_len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::MockFilterRuleRepository;
    use chrono::Utc;

    #[tokio::test]
    async fn test_mock_filter_rule_repository_create() {
        let repo = MockFilterRuleRepository::new();
        let now = Utc::now().naive_utc();
        let rule = NewFilterRule {
            target_type: FilterTargetType::Global,
            target_id: None,
            rule_order: 1,
            is_positive: true,
            regex_pattern: "test.*".to_string(),
            created_at: now,
            updated_at: now,
        };
        let created = repo.create(rule).await.unwrap();
        assert_eq!(created.rule_id, 1);
        assert_eq!(created.regex_pattern, "test.*");
    }

    #[tokio::test]
    async fn test_mock_filter_rule_repository_find_by_target() {
        let now = Utc::now().naive_utc();
        let rule1 = FilterRule {
            rule_id: 1,
            target_type: FilterTargetType::Global,
            target_id: None,
            rule_order: 1,
            is_positive: true,
            regex_pattern: "pattern1".to_string(),
            created_at: now,
            updated_at: now,
        };
        let rule2 = FilterRule {
            rule_id: 2,
            target_type: FilterTargetType::Anime,
            target_id: Some(10),
            rule_order: 1,
            is_positive: false,
            regex_pattern: "pattern2".to_string(),
            created_at: now,
            updated_at: now,
        };
        let repo = MockFilterRuleRepository::with_data(vec![rule1, rule2]);

        let global_rules = repo.find_by_target(FilterTargetType::Global, None).await.unwrap();
        assert_eq!(global_rules.len(), 1);

        let sub_rules = repo.find_by_target(FilterTargetType::Anime, Some(10)).await.unwrap();
        assert_eq!(sub_rules.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_filter_rule_repository_delete() {
        let now = Utc::now().naive_utc();
        let rule = FilterRule {
            rule_id: 1,
            target_type: FilterTargetType::Global,
            target_id: None,
            rule_order: 1,
            is_positive: true,
            regex_pattern: "test".to_string(),
            created_at: now,
            updated_at: now,
        };
        let repo = MockFilterRuleRepository::with_data(vec![rule]);

        let deleted = repo.delete(1).await.unwrap();
        assert!(deleted);

        let not_deleted = repo.delete(999).await.unwrap();
        assert!(!not_deleted);
    }
}
