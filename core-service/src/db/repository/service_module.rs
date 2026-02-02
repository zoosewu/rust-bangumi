use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::{ModuleTypeEnum, NewServiceModule, ServiceModule};
use crate::schema::service_modules;
use super::RepositoryError;

#[async_trait]
pub trait ServiceModuleRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<ServiceModule>, RepositoryError>;
    async fn find_by_name(&self, name: &str) -> Result<Option<ServiceModule>, RepositoryError>;
    async fn find_by_type(&self, module_type: ModuleTypeEnum) -> Result<Vec<ServiceModule>, RepositoryError>;
    async fn find_enabled(&self) -> Result<Vec<ServiceModule>, RepositoryError>;
    async fn find_all(&self) -> Result<Vec<ServiceModule>, RepositoryError>;
    async fn create(&self, new_module: NewServiceModule) -> Result<ServiceModule, RepositoryError>;
    async fn upsert_by_name(&self, new_module: NewServiceModule) -> Result<ServiceModule, RepositoryError>;
    async fn update_enabled(&self, id: i32, is_enabled: bool) -> Result<ServiceModule, RepositoryError>;
    async fn delete(&self, id: i32) -> Result<bool, RepositoryError>;
}

pub struct DieselServiceModuleRepository {
    pool: DbPool,
}

impl DieselServiceModuleRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ServiceModuleRepository for DieselServiceModuleRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<ServiceModule>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            service_modules::table
                .find(id)
                .first::<ServiceModule>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<ServiceModule>, RepositoryError> {
        let pool = self.pool.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            service_modules::table
                .filter(service_modules::name.eq(&name))
                .first::<ServiceModule>(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_type(&self, module_type: ModuleTypeEnum) -> Result<Vec<ServiceModule>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            service_modules::table
                .filter(service_modules::module_type.eq(module_type))
                .order(service_modules::priority.desc())
                .load::<ServiceModule>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_enabled(&self) -> Result<Vec<ServiceModule>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            service_modules::table
                .filter(service_modules::is_enabled.eq(true))
                .order(service_modules::priority.desc())
                .load::<ServiceModule>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_all(&self) -> Result<Vec<ServiceModule>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            service_modules::table
                .order(service_modules::priority.desc())
                .load::<ServiceModule>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn create(&self, new_module: NewServiceModule) -> Result<ServiceModule, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::insert_into(service_modules::table)
                .values(&new_module)
                .get_result::<ServiceModule>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn upsert_by_name(&self, new_module: NewServiceModule) -> Result<ServiceModule, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;

            // Check if exists
            let existing = service_modules::table
                .filter(service_modules::name.eq(&new_module.name))
                .first::<ServiceModule>(&mut conn)
                .optional()?;

            if let Some(existing) = existing {
                // Update existing
                diesel::update(service_modules::table.find(existing.module_id))
                    .set((
                        service_modules::module_type.eq(&new_module.module_type),
                        service_modules::version.eq(&new_module.version),
                        service_modules::description.eq(&new_module.description),
                        service_modules::is_enabled.eq(new_module.is_enabled),
                        service_modules::base_url.eq(&new_module.base_url),
                        service_modules::updated_at.eq(Utc::now().naive_utc()),
                    ))
                    .get_result::<ServiceModule>(&mut conn)
                    .map_err(RepositoryError::from)
            } else {
                // Insert new
                diesel::insert_into(service_modules::table)
                    .values(&new_module)
                    .get_result::<ServiceModule>(&mut conn)
                    .map_err(RepositoryError::from)
            }
        })
        .await?
    }

    async fn update_enabled(&self, id: i32, is_enabled: bool) -> Result<ServiceModule, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            diesel::update(service_modules::table.find(id))
                .set((
                    service_modules::is_enabled.eq(is_enabled),
                    service_modules::updated_at.eq(Utc::now().naive_utc()),
                ))
                .get_result::<ServiceModule>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let rows_deleted = diesel::delete(service_modules::table.find(id))
                .execute(&mut conn)?;
            Ok(rows_deleted > 0)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockServiceModuleRepository {
        pub modules: Mutex<Vec<ServiceModule>>,
        pub operations: Mutex<Vec<String>>,
    }

    impl MockServiceModuleRepository {
        pub fn new() -> Self {
            Self {
                modules: Mutex::new(Vec::new()),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn with_data(modules: Vec<ServiceModule>) -> Self {
            Self {
                modules: Mutex::new(modules),
                operations: Mutex::new(Vec::new()),
            }
        }

        pub fn get_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }
    }

    impl Default for MockServiceModuleRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl ServiceModuleRepository for MockServiceModuleRepository {
        async fn find_by_id(&self, id: i32) -> Result<Option<ServiceModule>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_id:{}", id));
            Ok(self.modules.lock().unwrap()
                .iter()
                .find(|m| m.module_id == id)
                .cloned())
        }

        async fn find_by_name(&self, name: &str) -> Result<Option<ServiceModule>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_name:{}", name));
            Ok(self.modules.lock().unwrap()
                .iter()
                .find(|m| m.name == name)
                .cloned())
        }

        async fn find_by_type(&self, module_type: ModuleTypeEnum) -> Result<Vec<ServiceModule>, RepositoryError> {
            self.operations.lock().unwrap().push(format!("find_by_type:{:?}", module_type));
            Ok(self.modules.lock().unwrap()
                .iter()
                .filter(|m| m.module_type == module_type)
                .cloned()
                .collect())
        }

        async fn find_enabled(&self) -> Result<Vec<ServiceModule>, RepositoryError> {
            self.operations.lock().unwrap().push("find_enabled".to_string());
            Ok(self.modules.lock().unwrap()
                .iter()
                .filter(|m| m.is_enabled)
                .cloned()
                .collect())
        }

        async fn find_all(&self) -> Result<Vec<ServiceModule>, RepositoryError> {
            self.operations.lock().unwrap().push("find_all".to_string());
            Ok(self.modules.lock().unwrap().clone())
        }

        async fn create(&self, new_module: NewServiceModule) -> Result<ServiceModule, RepositoryError> {
            self.operations.lock().unwrap().push(format!("create:{}", new_module.name));
            let mut modules = self.modules.lock().unwrap();
            let id = modules.len() as i32 + 1;
            let module = ServiceModule {
                module_id: id,
                module_type: new_module.module_type,
                name: new_module.name,
                version: new_module.version,
                description: new_module.description,
                is_enabled: new_module.is_enabled,
                config_schema: new_module.config_schema,
                priority: new_module.priority,
                base_url: new_module.base_url,
                created_at: new_module.created_at,
                updated_at: new_module.updated_at,
            };
            modules.push(module.clone());
            Ok(module)
        }

        async fn upsert_by_name(&self, new_module: NewServiceModule) -> Result<ServiceModule, RepositoryError> {
            self.operations.lock().unwrap().push(format!("upsert_by_name:{}", new_module.name));
            let mut modules = self.modules.lock().unwrap();

            if let Some(pos) = modules.iter().position(|m| m.name == new_module.name) {
                modules[pos].module_type = new_module.module_type;
                modules[pos].version = new_module.version;
                modules[pos].description = new_module.description;
                modules[pos].is_enabled = new_module.is_enabled;
                modules[pos].base_url = new_module.base_url;
                modules[pos].updated_at = Utc::now().naive_utc();
                Ok(modules[pos].clone())
            } else {
                let id = modules.len() as i32 + 1;
                let module = ServiceModule {
                    module_id: id,
                    module_type: new_module.module_type,
                    name: new_module.name,
                    version: new_module.version,
                    description: new_module.description,
                    is_enabled: new_module.is_enabled,
                    config_schema: new_module.config_schema,
                    priority: new_module.priority,
                    base_url: new_module.base_url,
                    created_at: new_module.created_at,
                    updated_at: new_module.updated_at,
                };
                modules.push(module.clone());
                Ok(module)
            }
        }

        async fn update_enabled(&self, id: i32, is_enabled: bool) -> Result<ServiceModule, RepositoryError> {
            self.operations.lock().unwrap().push(format!("update_enabled:{}:{}", id, is_enabled));
            let mut modules = self.modules.lock().unwrap();
            if let Some(pos) = modules.iter().position(|m| m.module_id == id) {
                modules[pos].is_enabled = is_enabled;
                modules[pos].updated_at = Utc::now().naive_utc();
                Ok(modules[pos].clone())
            } else {
                Err(RepositoryError::NotFound)
            }
        }

        async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
            self.operations.lock().unwrap().push(format!("delete:{}", id));
            let mut modules = self.modules.lock().unwrap();
            let len_before = modules.len();
            modules.retain(|m| m.module_id != id);
            Ok(modules.len() < len_before)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn create_test_module(id: i32, name: &str, module_type: ModuleTypeEnum, is_enabled: bool) -> ServiceModule {
            let now = Utc::now().naive_utc();
            ServiceModule {
                module_id: id,
                module_type,
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: Some(format!("Test module {}", name)),
                is_enabled,
                config_schema: None,
                priority: 50,
                base_url: format!("http://localhost:800{}", id),
                created_at: now,
                updated_at: now,
            }
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_find_by_id() {
            let module = create_test_module(1, "fetcher1", ModuleTypeEnum::Fetcher, true);
            let repo = MockServiceModuleRepository::with_data(vec![module]);

            let found = repo.find_by_id(1).await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().name, "fetcher1");

            let not_found = repo.find_by_id(999).await.unwrap();
            assert!(not_found.is_none());
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_find_by_name() {
            let module = create_test_module(1, "mikanani", ModuleTypeEnum::Fetcher, true);
            let repo = MockServiceModuleRepository::with_data(vec![module]);

            let found = repo.find_by_name("mikanani").await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().module_id, 1);

            let not_found = repo.find_by_name("nonexistent").await.unwrap();
            assert!(not_found.is_none());
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_find_by_type() {
            let fetcher1 = create_test_module(1, "fetcher1", ModuleTypeEnum::Fetcher, true);
            let fetcher2 = create_test_module(2, "fetcher2", ModuleTypeEnum::Fetcher, true);
            let downloader = create_test_module(3, "downloader1", ModuleTypeEnum::Downloader, true);
            let repo = MockServiceModuleRepository::with_data(vec![fetcher1, fetcher2, downloader]);

            let fetchers = repo.find_by_type(ModuleTypeEnum::Fetcher).await.unwrap();
            assert_eq!(fetchers.len(), 2);

            let downloaders = repo.find_by_type(ModuleTypeEnum::Downloader).await.unwrap();
            assert_eq!(downloaders.len(), 1);

            let viewers = repo.find_by_type(ModuleTypeEnum::Viewer).await.unwrap();
            assert_eq!(viewers.len(), 0);
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_find_enabled() {
            let enabled = create_test_module(1, "enabled", ModuleTypeEnum::Fetcher, true);
            let disabled = create_test_module(2, "disabled", ModuleTypeEnum::Fetcher, false);
            let repo = MockServiceModuleRepository::with_data(vec![enabled, disabled]);

            let enabled_modules = repo.find_enabled().await.unwrap();
            assert_eq!(enabled_modules.len(), 1);
            assert_eq!(enabled_modules[0].name, "enabled");
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_create() {
            let repo = MockServiceModuleRepository::new();
            let now = Utc::now().naive_utc();

            let new_module = NewServiceModule {
                module_type: ModuleTypeEnum::Fetcher,
                name: "new_fetcher".to_string(),
                version: "1.0.0".to_string(),
                description: Some("New fetcher".to_string()),
                is_enabled: true,
                config_schema: None,
                priority: 50,
                base_url: "http://localhost:8001".to_string(),
                created_at: now,
                updated_at: now,
            };

            let created = repo.create(new_module).await.unwrap();
            assert_eq!(created.module_id, 1);
            assert_eq!(created.name, "new_fetcher");

            let ops = repo.get_operations();
            assert_eq!(ops, vec!["create:new_fetcher"]);
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_upsert_by_name() {
            let existing = create_test_module(1, "existing", ModuleTypeEnum::Fetcher, true);
            let repo = MockServiceModuleRepository::with_data(vec![existing]);
            let now = Utc::now().naive_utc();

            // Update existing
            let update_module = NewServiceModule {
                module_type: ModuleTypeEnum::Fetcher,
                name: "existing".to_string(),
                version: "2.0.0".to_string(),
                description: Some("Updated".to_string()),
                is_enabled: false,
                config_schema: None,
                priority: 100,
                base_url: "http://localhost:9000".to_string(),
                created_at: now,
                updated_at: now,
            };

            let updated = repo.upsert_by_name(update_module).await.unwrap();
            assert_eq!(updated.module_id, 1); // Same ID
            assert_eq!(updated.version, "2.0.0");
            assert!(!updated.is_enabled);

            // Insert new
            let new_module = NewServiceModule {
                module_type: ModuleTypeEnum::Downloader,
                name: "new_downloader".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                is_enabled: true,
                config_schema: None,
                priority: 50,
                base_url: "http://localhost:8002".to_string(),
                created_at: now,
                updated_at: now,
            };

            let inserted = repo.upsert_by_name(new_module).await.unwrap();
            assert_eq!(inserted.module_id, 2); // New ID
            assert_eq!(inserted.name, "new_downloader");
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_update_enabled() {
            let module = create_test_module(1, "test", ModuleTypeEnum::Fetcher, true);
            let repo = MockServiceModuleRepository::with_data(vec![module]);

            let updated = repo.update_enabled(1, false).await.unwrap();
            assert!(!updated.is_enabled);

            let result = repo.update_enabled(999, true).await;
            assert!(matches!(result, Err(RepositoryError::NotFound)));
        }

        #[tokio::test]
        async fn test_mock_service_module_repository_delete() {
            let module = create_test_module(1, "to_delete", ModuleTypeEnum::Fetcher, true);
            let repo = MockServiceModuleRepository::with_data(vec![module]);

            let deleted = repo.delete(1).await.unwrap();
            assert!(deleted);

            let not_deleted = repo.delete(999).await.unwrap();
            assert!(!not_deleted);

            // Verify it's actually deleted
            let found = repo.find_by_id(1).await.unwrap();
            assert!(found.is_none());
        }
    }
}
