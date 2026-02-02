use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use diesel_migrations::{FileBasedMigrations, MigrationHarness};

pub mod models;
pub mod repository;

pub use repository::{
    RepositoryError,
    AnimeRepository, DieselAnimeRepository,
    SubscriptionRepository, DieselSubscriptionRepository,
    ServiceModuleRepository, DieselServiceModuleRepository,
    SeasonRepository, DieselSeasonRepository,
    AnimeSeriesRepository, DieselAnimeSeriesRepository, CreateAnimeSeriesParams,
    SubtitleGroupRepository, DieselSubtitleGroupRepository,
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
    let mut conn = pool.get()
        .map_err(|e| anyhow::anyhow!("Failed to get connection from pool: {}", e))?;

    let migrations = FileBasedMigrations::from_path("./migrations")
        .map_err(|e| anyhow::anyhow!("Failed to load migrations: {}", e))?;

    conn.run_pending_migrations(migrations)
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    Ok(())
}
