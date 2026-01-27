use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde_json::json;
use shared::{ServiceRegistration, ServiceRegistrationResponse, ServiceType};
use uuid::Uuid;
use crate::state::AppState;
use crate::schema::fetcher_modules;
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

    // If this is a Fetcher service, persist it to the database
    if payload.service_type == ServiceType::Fetcher {
        let naive_now = now.naive_utc();

        // Get a connection from the pool and insert the fetcher module
        match state.db.get() {
            Ok(mut conn) => {
                use crate::schema::fetcher_modules::dsl::*;

                let fetcher_base_url = format!("http://{}:{}", payload.host, payload.port);
                let insert_query = diesel::sql_query(
                    "INSERT INTO fetcher_modules (name, version, description, is_enabled, config_schema, created_at, updated_at, priority, base_url) \
                     VALUES ($1, $2, $3, $4, NULL::jsonb, $5, $6, $7, $8)"
                )
                .bind::<diesel::sql_types::Varchar, _>(&payload.service_name)
                .bind::<diesel::sql_types::Varchar, _>("1.0.0")
                .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(
                    Some(format!("{}:{}:{}", payload.service_name, payload.host, payload.port))
                )
                .bind::<diesel::sql_types::Bool, _>(true)
                .bind::<diesel::sql_types::Timestamp, _>(naive_now)
                .bind::<diesel::sql_types::Timestamp, _>(naive_now)
                .bind::<diesel::sql_types::Int4, _>(50i32) // Default priority
                .bind::<diesel::sql_types::Text, _>(&fetcher_base_url); // base_url

                match insert_query.execute(&mut conn) {
                    Ok(_) => {
                        tracing::info!(
                            "Successfully persisted Fetcher service to database: {} ({}:{})",
                            payload.service_name,
                            payload.host,
                            payload.port
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to insert Fetcher module into database: {}",
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
