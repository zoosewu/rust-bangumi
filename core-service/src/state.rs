use crate::db::{
    AnimeLinkConflictRepository, AnimeLinkRepository, AnimeRepository, AnimeSeriesRepository,
    ConflictRepository, DbPool, DieselAnimeLinkConflictRepository, DieselAnimeLinkRepository,
    DieselAnimeRepository, DieselAnimeSeriesRepository, DieselConflictRepository,
    DieselFilterRuleRepository, DieselRawItemRepository, DieselSeasonRepository,
    DieselServiceModuleRepository, DieselSubscriptionRepository, DieselSubtitleGroupRepository,
    DieselTitleParserRepository, FilterRuleRepository, RawItemRepository, SeasonRepository,
    ServiceModuleRepository, SubscriptionRepository, SubtitleGroupRepository,
    TitleParserRepository,
};
use crate::services::{
    ConflictDetectionService, DownloadDispatchService, ServiceRegistry, SyncService,
};
use std::sync::Arc;

pub struct Repositories {
    pub anime: Arc<dyn AnimeRepository>,
    pub subscription: Arc<dyn SubscriptionRepository>,
    pub service_module: Arc<dyn ServiceModuleRepository>,
    pub season: Arc<dyn SeasonRepository>,
    pub anime_series: Arc<dyn AnimeSeriesRepository>,
    pub subtitle_group: Arc<dyn SubtitleGroupRepository>,
    pub filter_rule: Arc<dyn FilterRuleRepository>,
    pub anime_link: Arc<dyn AnimeLinkRepository>,
    pub title_parser: Arc<dyn TitleParserRepository>,
    pub raw_item: Arc<dyn RawItemRepository>,
    pub conflict: Arc<dyn ConflictRepository>,
    pub anime_link_conflict: Arc<dyn AnimeLinkConflictRepository>,
}

impl Repositories {
    pub fn new(pool: DbPool) -> Self {
        Self {
            anime: Arc::new(DieselAnimeRepository::new(pool.clone())),
            subscription: Arc::new(DieselSubscriptionRepository::new(pool.clone())),
            service_module: Arc::new(DieselServiceModuleRepository::new(pool.clone())),
            season: Arc::new(DieselSeasonRepository::new(pool.clone())),
            anime_series: Arc::new(DieselAnimeSeriesRepository::new(pool.clone())),
            subtitle_group: Arc::new(DieselSubtitleGroupRepository::new(pool.clone())),
            filter_rule: Arc::new(DieselFilterRuleRepository::new(pool.clone())),
            anime_link: Arc::new(DieselAnimeLinkRepository::new(pool.clone())),
            title_parser: Arc::new(DieselTitleParserRepository::new(pool.clone())),
            raw_item: Arc::new(DieselRawItemRepository::new(pool.clone())),
            conflict: Arc::new(DieselConflictRepository::new(pool.clone())),
            anime_link_conflict: Arc::new(DieselAnimeLinkConflictRepository::new(pool)),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub registry: Arc<ServiceRegistry>,
    pub repos: Arc<Repositories>,
    pub dispatch_service: Arc<DownloadDispatchService>,
    pub sync_service: Arc<SyncService>,
    pub conflict_detection: Arc<ConflictDetectionService>,
}

impl AppState {
    pub fn new(db: DbPool, registry: ServiceRegistry) -> Self {
        let repos = Repositories::new(db.clone());
        let dispatch_service = DownloadDispatchService::new(db.clone());
        let sync_service = SyncService::new(db.clone());
        let conflict_detection = ConflictDetectionService::new(
            repos.anime_link.clone(),
            repos.anime_link_conflict.clone(),
        );
        Self {
            db,
            registry: Arc::new(registry),
            repos: Arc::new(repos),
            dispatch_service: Arc::new(dispatch_service),
            sync_service: Arc::new(sync_service),
            conflict_detection: Arc::new(conflict_detection),
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
        filter_rule: Arc<dyn FilterRuleRepository>,
        anime_link: Arc<dyn AnimeLinkRepository>,
        title_parser: Arc<dyn TitleParserRepository>,
        raw_item: Arc<dyn RawItemRepository>,
        conflict: Arc<dyn ConflictRepository>,
        anime_link_conflict: Arc<dyn AnimeLinkConflictRepository>,
    ) -> Self {
        Self {
            anime,
            subscription,
            service_module,
            season,
            anime_series,
            subtitle_group,
            filter_rule,
            anime_link,
            title_parser,
            raw_item,
            conflict,
            anime_link_conflict,
        }
    }
}
