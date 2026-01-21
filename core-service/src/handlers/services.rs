use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde_json::json;
use shared::{ServiceRegistration, ServiceRegistrationResponse, ServiceType};
use uuid::Uuid;
use crate::state::AppState;

/// Register a new service
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<ServiceRegistration>,
) -> (StatusCode, Json<ServiceRegistrationResponse>) {
    let service_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let service = shared::RegisteredService {
        service_id,
        service_type: payload.service_type,
        service_name: payload.service_name,
        host: payload.host,
        port: payload.port,
        capabilities: payload.capabilities,
        is_healthy: true,
        last_heartbeat: now,
    };

    if let Err(e) = state.registry.register(service) {
        tracing::error!("Failed to register service: {}", e);
    }

    let response = ServiceRegistrationResponse {
        service_id,
        registered_at: now,
    };

    (StatusCode::CREATED, Json(response))
}

/// List all registered services
pub async fn list_services(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    match state.registry.get_services() {
        Ok(services) => Json(json!({ "services": services })),
        Err(e) => {
            tracing::error!("Failed to get services: {}", e);
            Json(json!({ "services": [] }))
        }
    }
}

/// List services by type (fetcher, downloader, viewer)
pub async fn list_by_type(
    State(state): State<AppState>,
    Path(service_type): Path<String>,
) -> Json<serde_json::Value> {
    let service_type_enum = match service_type.to_lowercase().as_str() {
        "fetcher" => ServiceType::Fetcher,
        "downloader" => ServiceType::Downloader,
        "viewer" => ServiceType::Viewer,
        _ => {
            return Json(json!({"error": "Invalid service type", "services": []}))
        }
    };

    match state.registry.get_services_by_type(&service_type_enum) {
        Ok(services) => Json(json!({ "services": services })),
        Err(e) => {
            tracing::error!("Failed to get services by type: {}", e);
            Json(json!({ "services": [] }))
        }
    }
}

/// Update service health status
pub async fn health_check(
    State(state): State<AppState>,
    Path(service_id): Path<Uuid>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.registry.update_health(service_id, true) {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))),
        Err(e) => {
            tracing::error!("Failed to update health: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"status": "error", "message": e})))
        }
    }
}
