//! Integration tests for Anime API endpoints
//!
//! These tests verify the complete CRUD operations for the Anime API,
//! including database interactions and error handling.

use chrono::{NaiveDate, Utc};
use diesel::prelude::*;

// Re-export modules from the core-service library
use core_service::models::*;
use core_service::schema::*;

type DbPool = diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

/// Setup helper: Establish test database connection pool
///
/// This function creates a connection pool pointing to a test database.
/// The database URL should be provided via DATABASE_TEST_URL environment variable.
/// Falls back to localhost if not set.
fn setup_test_db() -> Result<DbPool, String> {
    let database_url = std::env::var("DATABASE_TEST_URL").unwrap_or_else(|_| {
        "postgresql://bangumi:bangumi_password@localhost:5432/bangumi_test".to_string()
    });

    let manager = diesel::r2d2::ConnectionManager::<diesel::PgConnection>::new(&database_url);
    let pool = diesel::r2d2::Pool::builder()
        .max_size(2)
        .build(manager)
        .map_err(|e| format!("Failed to create test connection pool: {}", e))?;

    // Verify connection is working
    pool.get()
        .map_err(|e| format!("Failed to connect to test database: {}", e))?;

    Ok(pool)
}

// ============ AnimeWork CRUD Tests ============

/// Test: Create a new anime work
///
/// Verifies that an anime work record can be created successfully in the database
/// with all required fields populated.
#[test]
#[ignore]
fn test_create_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime Create".to_string(),
        created_at: now,
        updated_at: now,
    };

    let result = diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    assert_eq!(result.title, "Test Anime Create");
    assert!(result.work_id > 0);
    println!("Created anime work with ID: {}", result.work_id);

    Ok(())
}

/// Test: Retrieve anime work by ID
///
/// Verifies that an anime work can be retrieved from the database by its primary key
/// and all fields match the original data.
#[test]
#[ignore]
fn test_get_anime_by_id() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // First create an anime work
    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime Get".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    let work_id = created.work_id;

    // Now retrieve it
    let retrieved = anime_works::table
        .find(work_id)
        .first::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to retrieve anime work: {}", e))?;

    assert_eq!(retrieved.work_id, work_id);
    assert_eq!(retrieved.title, "Test Anime Get");
    println!("Retrieved anime work: {:?}", retrieved);

    Ok(())
}

/// Test: List all anime works
///
/// Verifies that all anime work records can be retrieved from the database
/// and returns a non-empty result after creating test data.
#[test]
#[ignore]
fn test_get_all_animes() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create a test anime work
    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime List".to_string(),
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .execute(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    // Load all anime works
    let all_anime_works = anime_works::table
        .load::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to load anime works: {}", e))?;

    assert!(!all_anime_works.is_empty());
    println!("Found {} anime works in database", all_anime_works.len());

    Ok(())
}

/// Test: Update anime work
///
/// Verifies that an anime work record can be updated with new values
/// and the changes persist in the database.
#[test]
#[ignore]
fn test_update_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create an anime work
    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime Update Original".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    let work_id = created.work_id;

    // Update the anime work
    let updated = diesel::update(anime_works::table.find(work_id))
        .set((
            anime_works::title.eq("Test Anime Update Modified"),
            anime_works::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to update anime work: {}", e))?;

    assert_eq!(updated.title, "Test Anime Update Modified");
    assert_eq!(updated.work_id, work_id);
    println!("Updated anime work title: {}", updated.title);

    Ok(())
}

/// Test: Delete anime work
///
/// Verifies that an anime work record can be deleted from the database
/// and subsequent retrieval attempts fail as expected.
#[test]
#[ignore]
fn test_delete_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create an anime work
    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime Delete".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    let work_id = created.work_id;

    // Delete the anime work
    let deleted_count = diesel::delete(anime_works::table.find(work_id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete anime work: {}", e))?;

    assert_eq!(deleted_count, 1);

    // Verify it's deleted
    let result = anime_works::table
        .find(work_id)
        .first::<AnimeWork>(&mut conn)
        .optional()
        .map_err(|e| format!("Failed to query anime work: {}", e))?;

    assert!(result.is_none());
    println!("Successfully deleted anime work with ID: {}", work_id);

    Ok(())
}

// ============ Season CRUD Tests ============

/// Test: Create a new season
///
/// Verifies that a season record can be created successfully with year and season name.
#[test]
#[ignore]
fn test_create_season() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let new_season = NewSeason {
        year: 2024,
        season: "Winter".to_string(),
        created_at: Utc::now().naive_utc(),
    };

    let result = diesel::insert_into(seasons::table)
        .values(&new_season)
        .get_result::<Season>(&mut conn)
        .map_err(|e| format!("Failed to create season: {}", e))?;

    assert_eq!(result.year, 2024);
    assert_eq!(result.season, "Winter");
    assert!(result.season_id > 0);
    println!("Created season: {}/{}", result.year, result.season);

    Ok(())
}

/// Test: Get or create season (idempotent operation)
///
/// Verifies that the get_or_create operation correctly creates a new season
/// on first call and returns the existing season on subsequent calls with same parameters.
#[test]
#[ignore]
fn test_get_or_create_season() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let year = 2024;
    let season_name = "Spring".to_string();

    // First attempt - should create
    let existing_1 = seasons::table
        .filter(seasons::year.eq(year))
        .filter(seasons::season.eq(&season_name))
        .first::<Season>(&mut conn)
        .optional()
        .map_err(|e| format!("Failed to query season: {}", e))?;

    let season_1 = if let Some(s) = existing_1 {
        s
    } else {
        let new_season = NewSeason {
            year,
            season: season_name.clone(),
            created_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(seasons::table)
            .values(&new_season)
            .get_result::<Season>(&mut conn)
            .map_err(|e| format!("Failed to create season: {}", e))?
    };

    // Second attempt - should find existing
    let existing_2 = seasons::table
        .filter(seasons::year.eq(year))
        .filter(seasons::season.eq(&season_name))
        .first::<Season>(&mut conn)
        .optional()
        .map_err(|e| format!("Failed to query season: {}", e))?;

    let season_2 = if let Some(s) = existing_2 {
        s
    } else {
        return Err("Season should exist after first creation".to_string());
    };

    assert_eq!(season_1.season_id, season_2.season_id);
    println!(
        "Get or create season idempotence verified: ID {}",
        season_1.season_id
    );

    Ok(())
}

// ============ Anime Tests (formerly AnimeSeries) ============

/// Test: Create anime (series entry)
///
/// Verifies that an anime record can be created with proper foreign key relationships
/// and all optional fields are handled correctly.
#[test]
#[ignore]
fn test_create_anime_series() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create prerequisite anime work
    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime Series".to_string(),
        created_at: now,
        updated_at: now,
    };

    let anime_work = diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    // Create prerequisite season
    let new_season = NewSeason {
        year: 2024,
        season: "Summer".to_string(),
        created_at: now,
    };

    let season = diesel::insert_into(seasons::table)
        .values(&new_season)
        .get_result::<Season>(&mut conn)
        .map_err(|e| format!("Failed to create season: {}", e))?;

    // Now create anime
    let new_anime = NewAnime {
        work_id: anime_work.work_id,
        series_no: 1,
        season_id: season.season_id,
        description: Some("Test series description".to_string()),
        aired_date: Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2024, 9, 30).unwrap()),
        created_at: now,
        updated_at: now,
    };

    let result = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

    assert_eq!(result.work_id, anime_work.work_id);
    assert_eq!(result.series_no, 1);
    assert!(result.anime_id > 0);
    println!("Created anime with ID: {}", result.anime_id);

    Ok(())
}

/// Test: Get anime by work ID
///
/// Verifies that all anime entries associated with an anime work can be retrieved correctly.
#[test]
#[ignore]
fn test_get_anime_series_by_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create prerequisite anime work
    let now = Utc::now().naive_utc();
    let new_anime_work = NewAnimeWork {
        title: "Test Anime Multiple Series".to_string(),
        created_at: now,
        updated_at: now,
    };

    let anime_work = diesel::insert_into(anime_works::table)
        .values(&new_anime_work)
        .get_result::<AnimeWork>(&mut conn)
        .map_err(|e| format!("Failed to create anime work: {}", e))?;

    // Create prerequisite season
    let new_season = NewSeason {
        year: 2024,
        season: "Fall".to_string(),
        created_at: now,
    };

    let season = diesel::insert_into(seasons::table)
        .values(&new_season)
        .get_result::<Season>(&mut conn)
        .map_err(|e| format!("Failed to create season: {}", e))?;

    // Create two anime entries for the same anime work
    for series_no in 1..=2 {
        let new_anime = NewAnime {
            work_id: anime_work.work_id,
            series_no,
            season_id: season.season_id,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: now,
            updated_at: now,
        };

        diesel::insert_into(animes::table)
            .values(&new_anime)
            .execute(&mut conn)
            .map_err(|e| format!("Failed to create anime: {}", e))?;
    }

    // Retrieve all anime entries by work ID
    let anime_list = animes::table
        .filter(animes::work_id.eq(anime_work.work_id))
        .load::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to load anime list: {}", e))?;

    assert_eq!(anime_list.len(), 2);
    println!(
        "Retrieved {} anime entries for work ID {}",
        anime_list.len(),
        anime_work.work_id
    );

    Ok(())
}

// ============ SubtitleGroup Tests ============

/// Test: Create subtitle group
///
/// Verifies that a subtitle group record can be created successfully
/// with unique group name.
#[test]
#[ignore]
fn test_create_subtitle_group() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let new_group = NewSubtitleGroup {
        group_name: "Test Subtitle Group".to_string(),
        created_at: Utc::now().naive_utc(),
    };

    let result = diesel::insert_into(subtitle_groups::table)
        .values(&new_group)
        .get_result::<SubtitleGroup>(&mut conn)
        .map_err(|e| format!("Failed to create subtitle group: {}", e))?;

    assert_eq!(result.group_name, "Test Subtitle Group");
    assert!(result.group_id > 0);
    println!("Created subtitle group with ID: {}", result.group_id);

    Ok(())
}

// ============ Helper functions ============

/// Helper: Generate test anime title with timestamp
///
/// Useful for creating unique anime titles to avoid conflicts in tests
#[allow(dead_code)]
fn generate_unique_title(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        std::process::id(),
        Utc::now().timestamp()
    )
}

/// Helper: Generate test season name with timestamp
///
/// Useful for creating unique season combinations
#[allow(dead_code)]
fn generate_unique_season(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        std::process::id(),
        Utc::now().timestamp()
    )
}

/// Helper: Generate test subtitle group name with timestamp
///
/// Useful for creating unique group names
#[allow(dead_code)]
fn generate_unique_group_name(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        std::process::id(),
        Utc::now().timestamp()
    )
}
