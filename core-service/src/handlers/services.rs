use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde_json::json;
use shared::{ServiceRegistration, ServiceRegistrationResponse, ServiceType};
use uuid::Uuid;
use crate::state::AppState;
use crate::models::db::ModuleTypeEnum;
use diesel::prelude::*;

/// Register a new service
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<ServiceRegistration>,
) -> (StatusCode, Json<ServiceRegistrationResponse>) {
    let service_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let service = shared::RegisteredService {
        service_id,
        service_type: payload.service_type.clone(),
        service_name: payload.service_name.clone(),
        host: payload.host.clone(),
        port: payload.port,
        capabilities: payload.capabilities.clone(),
        is_healthy: true,
        last_heartbeat: now,
    };

    if let Err(e) = state.registry.register(service) {
        tracing::error!("Failed to register service: {}", e);
    }

    // Persist service registration to the database
    let naive_now = now.naive_utc();

    // Get a connection from the pool and persist to service_modules table
    match state.db.get() {
        Ok(mut conn) => {
            let service_base_url = format!("http://{}:{}", payload.host, payload.port);
            let service_description = format!("{}:{}:{}", payload.service_name, payload.host, payload.port);
            let module_type_str = ModuleTypeEnum::from(&payload.service_type).to_string();

            // Use UPSERT to handle service restart scenarios
            let upsert_query = diesel::sql_query(
                "INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, created_at, updated_at, priority, base_url) \
                 VALUES ($1::module_type, $2, $3, $4, $5, NULL, $6, $7, $8, $9) \
                 ON CONFLICT (name) DO UPDATE SET \
                 is_enabled = EXCLUDED.is_enabled, \
                 base_url = EXCLUDED.base_url, \
                 module_type = EXCLUDED.module_type, \
                 updated_at = EXCLUDED.updated_at"
            )
            .bind::<diesel::sql_types::Text, _>(&module_type_str)
            .bind::<diesel::sql_types::Varchar, _>(&payload.service_name)
            .bind::<diesel::sql_types::Varchar, _>("1.0.0")
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(&service_description))
            .bind::<diesel::sql_types::Bool, _>(true)
            .bind::<diesel::sql_types::Timestamp, _>(naive_now)
            .bind::<diesel::sql_types::Timestamp, _>(naive_now)
            .bind::<diesel::sql_types::Int4, _>(50i32) // Default priority
            .bind::<diesel::sql_types::Text, _>(&service_base_url);

            match upsert_query.execute(&mut conn) {
                Ok(_) => {
                    tracing::info!(
                        "Registered/Updated {} service in database: {} ({}:{})",
                        module_type_str,
                        payload.service_name,
                        payload.host,
                        payload.port
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to register {} module in database: {}",
                        module_type_str,
                        e
                    );
                }
            }
        }
        Err(e) => {
            tracing::error!(
                "Failed to get database connection: {}",
                e
            );
        }
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
