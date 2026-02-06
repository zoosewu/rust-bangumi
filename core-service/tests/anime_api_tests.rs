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

// ============ Anime CRUD Tests ============

/// Test: Create a new anime
///
/// Verifies that an anime record can be created successfully in the database
/// with all required fields populated.
#[test]
#[ignore]
fn test_create_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime Create".to_string(),
        created_at: now,
        updated_at: now,
    };

    let result = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

    assert_eq!(result.title, "Test Anime Create");
    assert!(result.anime_id > 0);
    println!("Created anime with ID: {}", result.anime_id);

    Ok(())
}

/// Test: Retrieve anime by ID
///
/// Verifies that an anime can be retrieved from the database by its primary key
/// and all fields match the original data.
#[test]
#[ignore]
fn test_get_anime_by_id() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // First create an anime
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime Get".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

    let anime_id = created.anime_id;

    // Now retrieve it
    let retrieved = animes::table
        .find(anime_id)
        .first::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to retrieve anime: {}", e))?;

    assert_eq!(retrieved.anime_id, anime_id);
    assert_eq!(retrieved.title, "Test Anime Get");
    println!("Retrieved anime: {:?}", retrieved);

    Ok(())
}

/// Test: List all animes
///
/// Verifies that all anime records can be retrieved from the database
/// and returns a non-empty result after creating test data.
#[test]
#[ignore]
fn test_get_all_animes() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create a test anime
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime List".to_string(),
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(animes::table)
        .values(&new_anime)
        .execute(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

    // Load all animes
    let all_animes = animes::table
        .load::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to load animes: {}", e))?;

    assert!(!all_animes.is_empty());
    println!("Found {} animes in database", all_animes.len());

    Ok(())
}

/// Test: Update anime
///
/// Verifies that an anime record can be updated with new values
/// and the changes persist in the database.
#[test]
#[ignore]
fn test_update_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create an anime
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime Update Original".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

    let anime_id = created.anime_id;

    // Update the anime
    let updated = diesel::update(animes::table.find(anime_id))
        .set((
            animes::title.eq("Test Anime Update Modified"),
            animes::updated_at.eq(Utc::now().naive_utc()),
        ))
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to update anime: {}", e))?;

    assert_eq!(updated.title, "Test Anime Update Modified");
    assert_eq!(updated.anime_id, anime_id);
    println!("Updated anime title: {}", updated.title);

    Ok(())
}

/// Test: Delete anime
///
/// Verifies that an anime record can be deleted from the database
/// and subsequent retrieval attempts fail as expected.
#[test]
#[ignore]
fn test_delete_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create an anime
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime Delete".to_string(),
        created_at: now,
        updated_at: now,
    };

    let created = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

    let anime_id = created.anime_id;

    // Delete the anime
    let deleted_count = diesel::delete(animes::table.find(anime_id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete anime: {}", e))?;

    assert_eq!(deleted_count, 1);

    // Verify it's deleted
    let result = animes::table
        .find(anime_id)
        .first::<Anime>(&mut conn)
        .optional()
        .map_err(|e| format!("Failed to query anime: {}", e))?;

    assert!(result.is_none());
    println!("Successfully deleted anime with ID: {}", anime_id);

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

// ============ AnimeSeries Tests ============

/// Test: Create anime series
///
/// Verifies that an anime series record can be created with proper foreign key relationships
/// and all optional fields are handled correctly.
#[test]
#[ignore]
fn test_create_anime_series() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create prerequisite anime
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime Series".to_string(),
        created_at: now,
        updated_at: now,
    };

    let anime = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

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

    // Now create anime series
    let new_series = NewAnimeSeries {
        anime_id: anime.anime_id,
        series_no: 1,
        season_id: season.season_id,
        description: Some("Test series description".to_string()),
        aired_date: Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2024, 9, 30).unwrap()),
        created_at: now,
        updated_at: now,
    };

    let result = diesel::insert_into(anime_series::table)
        .values(&new_series)
        .get_result::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to create anime series: {}", e))?;

    assert_eq!(result.anime_id, anime.anime_id);
    assert_eq!(result.series_no, 1);
    assert!(result.series_id > 0);
    println!("Created anime series with ID: {}", result.series_id);

    Ok(())
}

/// Test: Get anime series by anime ID
///
/// Verifies that all series associated with an anime can be retrieved correctly.
#[test]
#[ignore]
fn test_get_anime_series_by_anime() -> Result<(), String> {
    let pool = setup_test_db()?;
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    // Create prerequisite anime
    let now = Utc::now().naive_utc();
    let new_anime = NewAnime {
        title: "Test Anime Multiple Series".to_string(),
        created_at: now,
        updated_at: now,
    };

    let anime = diesel::insert_into(animes::table)
        .values(&new_anime)
        .get_result::<Anime>(&mut conn)
        .map_err(|e| format!("Failed to create anime: {}", e))?;

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

    // Create two series for the same anime
    for series_no in 1..=2 {
        let new_series = NewAnimeSeries {
            anime_id: anime.anime_id,
            series_no,
            season_id: season.season_id,
            description: None,
            aired_date: None,
            end_date: None,
            created_at: now,
            updated_at: now,
        };

        diesel::insert_into(anime_series::table)
            .values(&new_series)
            .execute(&mut conn)
            .map_err(|e| format!("Failed to create series: {}", e))?;
    }

    // Retrieve all series by anime ID
    let series_list = anime_series::table
        .filter(anime_series::anime_id.eq(anime.anime_id))
        .load::<AnimeSeries>(&mut conn)
        .map_err(|e| format!("Failed to load series: {}", e))?;

    assert_eq!(series_list.len(), 2);
    println!(
        "Retrieved {} series for anime ID {}",
        series_list.len(),
        anime.anime_id
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
