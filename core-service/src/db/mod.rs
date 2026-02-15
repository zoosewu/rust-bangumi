use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use diesel_migrations::{FileBasedMigrations, MigrationHarness};

pub mod models;
pub mod repository;

pub use repository::{
    AnimeLinkConflictRepository, AnimeLinkRepository, AnimeRepository, AnimeSeriesRepository,
    ConflictRepository, CreateAnimeSeriesParams, DieselAnimeLinkConflictRepository,
    DieselAnimeLinkRepository, DieselAnimeRepository, DieselAnimeSeriesRepository,
    DieselConflictRepository, DieselFilterRuleRepository, DieselRawItemRepository,
    DieselSeasonRepository, DieselServiceModuleRepository, DieselSubscriptionRepository,
    DieselSubtitleGroupRepository, DieselTitleParserRepository, FilterRuleRepository,
    RawItemFilter, RawItemRepository, RepositoryError, SeasonRepository, ServiceModuleRepository,
    SubscriptionRepository, SubtitleGroupRepository, TitleParserRepository,
};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

pub fn establish_connection_pool(database_url: &str) -> anyhow::Result<DbPool> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(16)
        .build(manager)
        .map_err(|e| anyhow::anyhow!("Failed to create connection pool: {}", e))?;

    Ok(pool)
}

pub fn run_migrations(pool: &DbPool) -> anyhow::Result<()> {
    let mut conn = pool
        .get()
        .map_err(|e| anyhow::anyhow!("Failed to get connection from pool: {}", e))?;

    // 使用相對於 Cargo.toml 的路徑，確保不受 CWD 影響
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let migrations_path = std::path::Path::new(manifest_dir).join("migrations");

    let migrations = FileBasedMigrations::from_path(migrations_path)
        .map_err(|e| anyhow::anyhow!("Failed to load migrations: {}", e))?;

    conn.run_pending_migrations(migrations)
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    Ok(())
}
