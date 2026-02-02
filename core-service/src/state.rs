use crate::services::ServiceRegistry;
use crate::db::{
    DbPool,
    AnimeRepository, DieselAnimeRepository,
    SubscriptionRepository, DieselSubscriptionRepository,
    ServiceModuleRepository, DieselServiceModuleRepository,
    SeasonRepository, DieselSeasonRepository,
    AnimeSeriesRepository, DieselAnimeSeriesRepository,
    SubtitleGroupRepository, DieselSubtitleGroupRepository,
};
use std::sync::Arc;

pub struct Repositories {
    pub anime: Arc<dyn AnimeRepository>,
    pub subscription: Arc<dyn SubscriptionRepository>,
    pub service_module: Arc<dyn ServiceModuleRepository>,
    pub season: Arc<dyn SeasonRepository>,
    pub anime_series: Arc<dyn AnimeSeriesRepository>,
    pub subtitle_group: Arc<dyn SubtitleGroupRepository>,
}

impl Repositories {
    pub fn new(pool: DbPool) -> Self {
        Self {
            anime: Arc::new(DieselAnimeRepository::new(pool.clone())),
            subscription: Arc::new(DieselSubscriptionRepository::new(pool.clone())),
            service_module: Arc::new(DieselServiceModuleRepository::new(pool.clone())),
            season: Arc::new(DieselSeasonRepository::new(pool.clone())),
            anime_series: Arc::new(DieselAnimeSeriesRepository::new(pool.clone())),
            subtitle_group: Arc::new(DieselSubtitleGroupRepository::new(pool)),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<ServiceRegistry>,
    pub repos: Arc<Repositories>,
}

impl AppState {
    pub fn new(db: DbPool, registry: ServiceRegistry) -> Self {
        let repos = Repositories::new(db.clone());
        Self {
            db,
            registry: Arc::new(registry),
            repos: Arc::new(repos),
        }
    }
}

#[cfg(test)]
impl Repositories {
    pub fn with_mocks(
        anime: Arc<dyn AnimeRepository>,
        subscription: Arc<dyn SubscriptionRepository>,
        service_module: Arc<dyn ServiceModuleRepository>,
        season: Arc<dyn SeasonRepository>,
        anime_series: Arc<dyn AnimeSeriesRepository>,
        subtitle_group: Arc<dyn SubtitleGroupRepository>,
    ) -> Self {
        Self {
            anime,
            subscription,
            service_module,
            season,
            anime_series,
            subtitle_group,
        }
    }
}
