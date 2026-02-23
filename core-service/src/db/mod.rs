use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

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

    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    Ok(())
}
