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

diesel::table! {
    fetcher_modules (fetcher_id) {
        fetcher_id -> Int4,
        name -> Varchar,
        version -> Varchar,
        description -> Nullable<Text>,
        is_enabled -> Bool,
        config_schema -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    rss_subscriptions (subscription_id) {
        subscription_id -> Int4,
        fetcher_id -> Int4,
        rss_url -> Varchar,
        name -> Nullable<Varchar>,
        description -> Nullable<Text>,
        last_fetched_at -> Nullable<Timestamp>,
        next_fetch_at -> Nullable<Timestamp>,
        fetch_interval_minutes -> Int4,
        is_active -> Bool,
        config -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    subscription_conflicts (conflict_id) {
        conflict_id -> Int4,
        subscription_id -> Int4,
        conflict_type -> Varchar,
        affected_item_id -> Nullable<Varchar>,
        conflict_data -> Text,
        resolution_status -> Varchar,
        resolution_data -> Nullable<Text>,
        created_at -> Timestamp,
        resolved_at -> Nullable<Timestamp>,
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
    fetcher_modules,
    rss_subscriptions,
    subscription_conflicts,
);
