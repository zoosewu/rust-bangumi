use chrono::Utc;
use shared::{RegisteredService, ServiceType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct ServiceRegistry {
    services: Arc<Mutex<HashMap<Uuid, RegisteredService>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, service: RegisteredService) -> Result<(), String> {
        let mut services = self.services.lock().map_err(|e| e.to_string())?;
        // Deduplicate by (host, port): remove any stale registration at the same
        // address so a restarted service doesn't accumulate duplicate entries.
        services.retain(|_, existing| {
            existing.host != service.host || existing.port != service.port
        });
        services.insert(service.service_id, service.clone());
        tracing::info!(
            "Service registered: {} ({})",
            service.service_name,
            service.service_type
        );
        Ok(())
    }

    pub fn get_services(&self) -> Result<Vec<RegisteredService>, String> {
        let services = self.services.lock().map_err(|e| e.to_string())?;
        Ok(services.values().cloned().collect())
    }

    pub fn get_services_by_type(
        &self,
        service_type: &ServiceType,
    ) -> Result<Vec<RegisteredService>, String> {
        let services = self.services.lock().map_err(|e| e.to_string())?;
        Ok(services
            .values()
            .filter(|s| &s.service_type == service_type)
            .cloned()
            .collect())
    }

    pub fn get_service(&self, service_id: Uuid) -> Result<Option<RegisteredService>, String> {
        let services = self.services.lock().map_err(|e| e.to_string())?;
        Ok(services.get(&service_id).cloned())
    }

    pub fn unregister(&self, service_id: Uuid) -> Result<(), String> {
        let mut services = self.services.lock().map_err(|e| e.to_string())?;
        services.remove(&service_id);
        tracing::info!("Service unregistered: {}", service_id);
        Ok(())
    }

    pub fn update_health(&self, service_id: Uuid, is_healthy: bool) -> Result<(), String> {
        let mut services = self.services.lock().map_err(|e| e.to_string())?;
        if let Some(service) = services.get_mut(&service_id) {
            service.is_healthy = is_healthy;
            service.last_heartbeat = Utc::now();
        }
        Ok(())
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ServiceRegistry {
    fn clone(&self) -> Self {
        Self {
            services: Arc::clone(&self.services),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{Capabilities, ServiceRegistration};

    fn create_test_service(id: Uuid) -> RegisteredService {
        RegisteredService {
            service_id: id,
            service_type: ServiceType::Fetcher,
            service_name: "test-fetcher".to_string(),
            host: "localhost".to_string(),
            port: 8001,
            capabilities: Capabilities {
                fetch_endpoint: Some("/fetch".to_string()),
                search_endpoint: None,
                detail_endpoint: None,
                download_endpoint: None,
                sync_endpoint: None,
                supported_download_types: vec![],
            },
            is_healthy: true,
            last_heartbeat: Utc::now(),
        }
    }

    #[test]
    fn test_register_service() {
        let registry = ServiceRegistry::new();
        let service_id = Uuid::new_v4();
        let service = create_test_service(service_id);

        assert!(registry.register(service).is_ok());
    }

    #[test]
    fn test_get_services() {
        let registry = ServiceRegistry::new();

        let mut service1 = create_test_service(Uuid::new_v4());
        service1.port = 8001;

        let mut service2 = create_test_service(Uuid::new_v4());
        service2.port = 8002; // different port → independent registration

        registry.register(service1).unwrap();
        registry.register(service2).unwrap();

        let services = registry.get_services().unwrap();
        assert_eq!(services.len(), 2);
    }

    #[test]
    fn test_unregister_service() {
        let registry = ServiceRegistry::new();
        let service_id = Uuid::new_v4();
        let service = create_test_service(service_id);

        registry.register(service).unwrap();
        assert_eq!(registry.get_services().unwrap().len(), 1);

        registry.unregister(service_id).unwrap();
        assert_eq!(registry.get_services().unwrap().len(), 0);
    }

    #[test]
    fn test_register_deduplicates_same_host_port() {
        // When the same service restarts (new UUID, same host:port), the old
        // registration must be replaced so search doesn't return duplicate results.
        let registry = ServiceRegistry::new();

        let id1 = Uuid::new_v4();
        let mut service1 = create_test_service(id1);
        service1.host = "localhost".to_string();
        service1.port = 8001;
        registry.register(service1).unwrap();
        assert_eq!(registry.get_services().unwrap().len(), 1);

        // Simulate restart: new UUID, same host:port
        let id2 = Uuid::new_v4();
        let mut service2 = create_test_service(id2);
        service2.host = "localhost".to_string();
        service2.port = 8001;
        registry.register(service2).unwrap();

        let services = registry.get_services().unwrap();
        assert_eq!(services.len(), 1, "Re-registering same host:port must replace old entry");
        assert_eq!(services[0].service_id, id2, "New registration should win");
    }

    #[test]
    fn test_register_different_ports_are_independent() {
        // Two services at different ports must both be kept.
        let registry = ServiceRegistry::new();

        let mut s1 = create_test_service(Uuid::new_v4());
        s1.host = "localhost".to_string();
        s1.port = 8001;

        let mut s2 = create_test_service(Uuid::new_v4());
        s2.host = "localhost".to_string();
        s2.port = 8002;

        registry.register(s1).unwrap();
        registry.register(s2).unwrap();

        assert_eq!(registry.get_services().unwrap().len(), 2);
    }

    #[test]
    fn test_update_health() {
        let registry = ServiceRegistry::new();
        let service_id = Uuid::new_v4();
        let mut service = create_test_service(service_id);
        service.is_healthy = true;

        registry.register(service).unwrap();

        registry.update_health(service_id, false).unwrap();
        let updated = registry.get_service(service_id).unwrap().unwrap();
        assert!(!updated.is_healthy);
    }
}
