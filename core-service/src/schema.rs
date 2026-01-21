// @generated automatically by Diesel CLI. (Manually generated since PostgreSQL not available)

diesel::table! {
    seasons (season_id) {
        season_id -> Int4,
        year -> Int4,
        season -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    animes (anime_id) {
        anime_id -> Int4,
        title -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    anime_series (series_id) {
        series_id -> Int4,
        anime_id -> Int4,
        series_no -> Int4,
        season_id -> Int4,
        description -> Nullable<Text>,
        aired_date -> Nullable<Date>,
        end_date -> Nullable<Date>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    subtitle_groups (group_id) {
        group_id -> Int4,
        group_name -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    anime_links (link_id) {
        link_id -> Int4,
        series_id -> Int4,
        group_id -> Int4,
        episode_no -> Int4,
        title -> Nullable<Varchar>,
        url -> Text,
        source_hash -> Varchar,
        filtered_flag -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    filter_rules (rule_id) {
        rule_id -> Int4,
        series_id -> Int4,
        group_id -> Int4,
        rule_order -> Int4,
        rule_type -> Varchar,
        regex_pattern -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    downloads (download_id) {
        download_id -> Int4,
        link_id -> Int4,
        downloader_type -> Varchar,
        status -> Varchar,
        progress -> Nullable<Float>,
        downloaded_bytes -> Nullable<Int8>,
        total_bytes -> Nullable<Int8>,
        error_message -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    cron_logs (log_id) {
        log_id -> Int4,
        fetcher_type -> Varchar,
        status -> Varchar,
        error_message -> Nullable<Text>,
        attempt_count -> Int4,
        executed_at -> Timestamp,
    }
}

// Foreign key relationships
diesel::allow_tables_to_appear_in_same_query!(
    seasons,
    animes,
    anime_series,
    subtitle_groups,
    anime_links,
    filter_rules,
    downloads,
    cron_logs,
);
