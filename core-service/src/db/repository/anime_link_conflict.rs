use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;

use super::RepositoryError;
use crate::db::DbPool;
use crate::models::AnimeLinkConflict;
use crate::schema::anime_link_conflicts;

#[async_trait]
pub trait AnimeLinkConflictRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeLinkConflict>, RepositoryError>;
    async fn find_unresolved(&self) -> Result<Vec<AnimeLinkConflict>, RepositoryError>;
    async fn find_by_episode(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<Option<AnimeLinkConflict>, RepositoryError>;
    async fn upsert(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<AnimeLinkConflict, RepositoryError>;
    async fn resolve(
        &self,
        conflict_id: i32,
        chosen_link_id: i32,
    ) -> Result<AnimeLinkConflict, RepositoryError>;
    async fn delete_by_episode(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<bool, RepositoryError>;
}

pub struct DieselAnimeLinkConflictRepository {
    pool: DbPool,
}

impl DieselAnimeLinkConflictRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnimeLinkConflictRepository for DieselAnimeLinkConflictRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<AnimeLinkConflict>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_link_conflicts::table
                .filter(anime_link_conflicts::conflict_id.eq(id))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_unresolved(&self) -> Result<Vec<AnimeLinkConflict>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_link_conflicts::table
                .filter(anime_link_conflicts::resolution_status.eq("unresolved"))
                .order(anime_link_conflicts::created_at.desc())
                .load::<AnimeLinkConflict>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn find_by_episode(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<Option<AnimeLinkConflict>, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            anime_link_conflicts::table
                .filter(anime_link_conflicts::series_id.eq(series_id))
                .filter(anime_link_conflicts::group_id.eq(group_id))
                .filter(anime_link_conflicts::episode_no.eq(episode_no))
                .first(&mut conn)
                .optional()
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn upsert(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<AnimeLinkConflict, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            diesel::insert_into(anime_link_conflicts::table)
                .values((
                    anime_link_conflicts::series_id.eq(series_id),
                    anime_link_conflicts::group_id.eq(group_id),
                    anime_link_conflicts::episode_no.eq(episode_no),
                    anime_link_conflicts::resolution_status.eq("unresolved"),
                    anime_link_conflicts::created_at.eq(now),
                ))
                .on_conflict((
                    anime_link_conflicts::series_id,
                    anime_link_conflicts::group_id,
                    anime_link_conflicts::episode_no,
                ))
                .do_update()
                .set(anime_link_conflicts::resolution_status.eq("unresolved"))
                .get_result::<AnimeLinkConflict>(&mut conn)
                .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn resolve(
        &self,
        conflict_id: i32,
        chosen_link_id: i32,
    ) -> Result<AnimeLinkConflict, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let now = Utc::now().naive_utc();
            diesel::update(
                anime_link_conflicts::table
                    .filter(anime_link_conflicts::conflict_id.eq(conflict_id)),
            )
            .set((
                anime_link_conflicts::resolution_status.eq("resolved"),
                anime_link_conflicts::chosen_link_id.eq(chosen_link_id),
                anime_link_conflicts::resolved_at.eq(now),
            ))
            .get_result::<AnimeLinkConflict>(&mut conn)
            .map_err(RepositoryError::from)
        })
        .await?
    }

    async fn delete_by_episode(
        &self,
        series_id: i32,
        group_id: i32,
        episode_no: i32,
    ) -> Result<bool, RepositoryError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let deleted = diesel::delete(
                anime_link_conflicts::table
                    .filter(anime_link_conflicts::series_id.eq(series_id))
                    .filter(anime_link_conflicts::group_id.eq(group_id))
                    .filter(anime_link_conflicts::episode_no.eq(episode_no)),
            )
            .execute(&mut conn)?;
            Ok(deleted > 0)
        })
        .await?
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    pub struct MockAnimeLinkConflictRepository {
        conflicts: Mutex<Vec<AnimeLinkConflict>>,
        next_id: Mutex<i32>,
    }

    impl MockAnimeLinkConflictRepository {
        pub fn new() -> Self {
            Self {
                conflicts: Mutex::new(Vec::new()),
                next_id: Mutex::new(1),
            }
        }
    }

    impl Default for MockAnimeLinkConflictRepository {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl AnimeLinkConflictRepository for MockAnimeLinkConflictRepository {
        async fn find_by_id(
            &self,
            id: i32,
        ) -> Result<Option<AnimeLinkConflict>, RepositoryError> {
            Ok(self
                .conflicts
                .lock()
                .unwrap()
                .iter()
                .find(|c| c.conflict_id == id)
                .cloned())
        }

        async fn find_unresolved(&self) -> Result<Vec<AnimeLinkConflict>, RepositoryError> {
            Ok(self
                .conflicts
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.resolution_status == "unresolved")
                .cloned()
                .collect())
        }

        async fn find_by_episode(
            &self,
            series_id: i32,
            group_id: i32,
            episode_no: i32,
        ) -> Result<Option<AnimeLinkConflict>, RepositoryError> {
            Ok(self
                .conflicts
                .lock()
                .unwrap()
                .iter()
                .find(|c| {
                    c.series_id == series_id
                        && c.group_id == group_id
                        && c.episode_no == episode_no
                })
                .cloned())
        }

        async fn upsert(
            &self,
            series_id: i32,
            group_id: i32,
            episode_no: i32,
        ) -> Result<AnimeLinkConflict, RepositoryError> {
            let mut conflicts = self.conflicts.lock().unwrap();
            if let Some(c) = conflicts.iter_mut().find(|c| {
                c.series_id == series_id && c.group_id == group_id && c.episode_no == episode_no
            }) {
                c.resolution_status = "unresolved".to_string();
                return Ok(c.clone());
            }
            let mut next_id = self.next_id.lock().unwrap();
            let conflict = AnimeLinkConflict {
                conflict_id: *next_id,
                series_id,
                group_id,
                episode_no,
                resolution_status: "unresolved".to_string(),
                chosen_link_id: None,
                created_at: Utc::now().naive_utc(),
                resolved_at: None,
            };
            *next_id += 1;
            conflicts.push(conflict.clone());
            Ok(conflict)
        }

        async fn resolve(
            &self,
            conflict_id: i32,
            chosen_link_id: i32,
        ) -> Result<AnimeLinkConflict, RepositoryError> {
            let mut conflicts = self.conflicts.lock().unwrap();
            if let Some(c) = conflicts
                .iter_mut()
                .find(|c| c.conflict_id == conflict_id)
            {
                c.resolution_status = "resolved".to_string();
                c.chosen_link_id = Some(chosen_link_id);
                c.resolved_at = Some(Utc::now().naive_utc());
                return Ok(c.clone());
            }
            Err(RepositoryError::NotFound)
        }

        async fn delete_by_episode(
            &self,
            series_id: i32,
            group_id: i32,
            episode_no: i32,
        ) -> Result<bool, RepositoryError> {
            let mut conflicts = self.conflicts.lock().unwrap();
            let orig = conflicts.len();
            conflicts.retain(|c| {
                !(c.series_id == series_id
                    && c.group_id == group_id
                    && c.episode_no == episode_no)
            });
            Ok(conflicts.len() < orig)
        }
    }
}
