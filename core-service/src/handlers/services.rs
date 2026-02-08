use crate::models::db::ModuleTypeEnum;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use diesel::prelude::*;
use serde_json::json;
use shared::{ServiceRegistration, ServiceRegistrationResponse, ServiceType};
use uuid::Uuid;

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
            let service_description =
                format!("{}:{}:{}", payload.service_name, payload.host, payload.port);
            let module_type_str = ModuleTypeEnum::from(&payload.service_type).to_string();

            // Use UPSERT to handle service restart scenarios
            let upsert_query = diesel::sql_query(
                "INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, created_at, updated_at, priority, base_url) \
                 VALUES ($1::module_type, $2, $3, $4, $5, NULL, $6, $7, $8, $9) \
                 ON CONFLICT (name) DO UPDATE SET \
                 is_enabled = EXCLUDED.is_enabled, \
                 base_url = EXCLUDED.base_url, \
                 module_type = EXCLUDED.module_type, \
                 version = EXCLUDED.version, \
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

                    // Save downloader capabilities if this is a downloader
                    if payload.service_type == ServiceType::Downloader
                        && !payload.capabilities.supported_download_types.is_empty()
                    {
                        save_downloader_capabilities(
                            &mut conn,
                            &payload.service_name,
                            &payload.capabilities.supported_download_types,
                        );

                        // Trigger retry of no_downloader links
                        let download_types: Vec<String> = payload
                            .capabilities
                            .supported_download_types
                            .iter()
                            .map(|dt| dt.to_string())
                            .collect();
                        let dispatch = state.dispatch_service.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                dispatch.retry_no_downloader_links(&download_types).await
                            {
                                tracing::error!("Failed to retry no_downloader links: {}", e);
                            }
                        });
                    }

                    // Trigger sync of completed downloads when a viewer registers
                    if payload.service_type == ServiceType::Viewer {
                        let sync_service = state.sync_service.clone();
                        tokio::spawn(async move {
                            if let Err(e) = sync_service.retry_completed_downloads().await {
                                tracing::error!(
                                    "Failed to retry completed downloads on viewer registration: {}",
                                    e
                                );
                            }
                        });
                    }
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
            tracing::error!("Failed to get database connection: {}", e);
        }
    }

    let response = ServiceRegistrationResponse {
        service_id,
        registered_at: now,
    };

    (StatusCode::CREATED, Json(response))
}

/// List all registered services
pub async fn list_services(State(state): State<AppState>) -> Json<serde_json::Value> {
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
        _ => return Json(json!({"error": "Invalid service type", "services": []})),
    };

    match state.registry.get_services_by_type(&service_type_enum) {
        Ok(services) => Json(json!({ "services": services })),
        Err(e) => {
            tracing::error!("Failed to get services by type: {}", e);
            Json(json!({ "services": [] }))
        }
    }
}

/// Save downloader capabilities to junction table
fn save_downloader_capabilities(
    conn: &mut diesel::PgConnection,
    service_name: &str,
    download_types: &[shared::DownloadType],
) {
    use crate::models::DownloaderCapability;
    use crate::schema::{downloader_capabilities, service_modules};

    // Get the module_id for this service
    let module_id: Option<i32> = service_modules::table
        .filter(service_modules::name.eq(service_name))
        .select(service_modules::module_id)
        .first::<i32>(conn)
        .ok();

    let Some(module_id) = module_id else {
        tracing::error!("Could not find module_id for service: {}", service_name);
        return;
    };

    // Delete existing capabilities
    if let Err(e) = diesel::delete(
        downloader_capabilities::table.filter(downloader_capabilities::module_id.eq(module_id)),
    )
    .execute(conn)
    {
        tracing::error!("Failed to delete old capabilities: {}", e);
        return;
    }

    // Insert new capabilities
    for dt in download_types {
        let cap = DownloaderCapability {
            module_id,
            download_type: dt.to_string(),
        };
        if let Err(e) = diesel::insert_into(downloader_capabilities::table)
            .values(&cap)
            .execute(conn)
        {
            tracing::error!("Failed to insert capability {}: {}", dt, e);
        }
    }

    tracing::info!(
        "Saved {} capabilities for downloader {}",
        download_types.len(),
        service_name
    );
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e})),
            )
        }
    }
}
