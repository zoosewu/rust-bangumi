use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use crate::{models::db::AnimeCoverImage, schema::anime_cover_images, state::AppState};

pub async fn list_anime_covers(
    State(state): State<AppState>,
    Path(anime_id): Path<i32>,
) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match anime_cover_images::table
        .filter(anime_cover_images::anime_id.eq(anime_id))
        .order(anime_cover_images::created_at.asc())
        .load::<AnimeCoverImage>(&mut conn)
    {
        Ok(list) => Json(list).into_response(),
        Err(e) => {
            tracing::error!("list_anime_covers failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn set_default_cover(
    State(state): State<AppState>,
    Path((anime_id, cover_id)): Path<(i32, i32)>,
) -> impl IntoResponse {
    let mut conn = match state.db.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let result = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        diesel::update(
            anime_cover_images::table.filter(anime_cover_images::anime_id.eq(anime_id)),
        )
        .set(anime_cover_images::is_default.eq(false))
        .execute(conn)?;

        let updated = diesel::update(
            anime_cover_images::table
                .filter(anime_cover_images::cover_id.eq(cover_id))
                .filter(anime_cover_images::anime_id.eq(anime_id)),
        )
        .set(anime_cover_images::is_default.eq(true))
        .execute(conn)?;

        if updated == 0 {
            return Err(diesel::result::Error::NotFound);
        }
        Ok(())
    });
    match result {
        Ok(_) => StatusCode::OK.into_response(),
        Err(diesel::result::Error::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("set_default_cover failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
