use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn create_pool(database_url: &str) -> DbPool {
    // 先測試連線，若資料庫不存在則自動建立
    if let Err(e) = PgConnection::establish(database_url) {
        let err_msg = e.to_string();
        if err_msg.contains("does not exist") {
            tracing::warn!("Database does not exist, attempting to create it...");
            if let Err(create_err) = create_database(database_url) {
                panic!("Failed to auto-create database: {}", create_err);
            }
        } else {
            panic!("Failed to connect to database: {}", e);
        }
    }

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .max_size(5)
        .build(manager)
        .expect("Failed to create database pool")
}

/// 連到同 host 的 `postgres` 預設庫，建立目標資料庫
fn create_database(database_url: &str) -> Result<(), String> {
    let (base, db_name) = database_url
        .rsplit_once('/')
        .ok_or_else(|| format!("Invalid database URL: {}", database_url))?;

    let db_name = db_name.split('?').next().unwrap_or(db_name);
    let maintenance_url = format!("{}/postgres", base);

    let mut conn = PgConnection::establish(&maintenance_url)
        .map_err(|e| format!("Failed to connect to maintenance database: {}", e))?;

    diesel::sql_query(format!(
        "CREATE DATABASE \"{}\"",
        db_name.replace('"', "\"\"")
    ))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to create database '{}': {}", db_name, e))?;

    tracing::info!("Auto-created database '{}'", db_name);
    Ok(())
}
