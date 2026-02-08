// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "filter_target_type"))]
    pub struct FilterTargetType;

    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "module_type"))]
    pub struct ModuleType;

    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "parser_source_type"))]
    pub struct ParserSourceType;
}

diesel::table! {
    anime_links (link_id) {
        link_id -> Int4,
        series_id -> Int4,
        group_id -> Int4,
        episode_no -> Int4,
        #[max_length = 255]
        title -> Nullable<Varchar>,
        url -> Text,
        #[max_length = 255]
        source_hash -> Varchar,
        filtered_flag -> Bool,
        created_at -> Timestamp,
        raw_item_id -> Nullable<Int4>,
        #[max_length = 20]
        download_type -> Nullable<Varchar>,
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
    animes (anime_id) {
        anime_id -> Int4,
        #[max_length = 255]
        title -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    cron_logs (log_id) {
        log_id -> Int4,
        #[max_length = 50]
        fetcher_type -> Varchar,
        #[max_length = 20]
        status -> Varchar,
        error_message -> Nullable<Text>,
        attempt_count -> Int4,
        executed_at -> Timestamp,
    }
}

diesel::table! {
    downloader_capabilities (module_id, download_type) {
        module_id -> Int4,
        #[max_length = 20]
        download_type -> Varchar,
    }
}

diesel::table! {
    downloads (download_id) {
        download_id -> Int4,
        link_id -> Int4,
        #[max_length = 50]
        downloader_type -> Varchar,
        #[max_length = 20]
        status -> Varchar,
        progress -> Nullable<Float4>,
        downloaded_bytes -> Nullable<Int8>,
        total_bytes -> Nullable<Int8>,
        error_message -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        module_id -> Nullable<Int4>,
        #[max_length = 255]
        torrent_hash -> Nullable<Varchar>,
        file_path -> Nullable<Text>,
        sync_retry_count -> Int4,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::FilterTargetType;

    filter_rules (rule_id) {
        rule_id -> Int4,
        rule_order -> Int4,
        regex_pattern -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        is_positive -> Bool,
        target_type -> FilterTargetType,
        target_id -> Nullable<Int4>,
    }
}

diesel::table! {
    raw_anime_items (item_id) {
        item_id -> Int4,
        title -> Text,
        description -> Nullable<Text>,
        #[max_length = 2048]
        download_url -> Varchar,
        pub_date -> Nullable<Timestamp>,
        subscription_id -> Int4,
        #[max_length = 20]
        status -> Varchar,
        parser_id -> Nullable<Int4>,
        error_message -> Nullable<Text>,
        parsed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    seasons (season_id) {
        season_id -> Int4,
        year -> Int4,
        #[max_length = 10]
        season -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ModuleType;

    service_modules (module_id) {
        module_id -> Int4,
        module_type -> ModuleType,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 50]
        version -> Varchar,
        description -> Nullable<Text>,
        is_enabled -> Bool,
        config_schema -> Nullable<Text>,
        priority -> Int4,
        #[max_length = 255]
        base_url -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    subscription_conflicts (conflict_id) {
        conflict_id -> Int4,
        subscription_id -> Int4,
        #[max_length = 50]
        conflict_type -> Varchar,
        #[max_length = 255]
        affected_item_id -> Nullable<Varchar>,
        conflict_data -> Jsonb,
        #[max_length = 50]
        resolution_status -> Varchar,
        resolution_data -> Nullable<Jsonb>,
        created_at -> Timestamp,
        resolved_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    subscriptions (subscription_id) {
        subscription_id -> Int4,
        fetcher_id -> Int4,
        #[max_length = 2048]
        source_url -> Varchar,
        #[max_length = 255]
        name -> Nullable<Varchar>,
        description -> Nullable<Text>,
        last_fetched_at -> Nullable<Timestamp>,
        next_fetch_at -> Nullable<Timestamp>,
        fetch_interval_minutes -> Int4,
        is_active -> Bool,
        config -> Nullable<Jsonb>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        #[max_length = 50]
        source_type -> Varchar,
        #[max_length = 20]
        assignment_status -> Varchar,
        assigned_at -> Nullable<Timestamp>,
        auto_selected -> Bool,
    }
}

diesel::table! {
    subtitle_groups (group_id) {
        group_id -> Int4,
        #[max_length = 255]
        group_name -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ParserSourceType;

    title_parsers (parser_id) {
        parser_id -> Int4,
        #[max_length = 100]
        name -> Varchar,
        description -> Nullable<Text>,
        priority -> Int4,
        is_enabled -> Bool,
        condition_regex -> Text,
        parse_regex -> Text,
        anime_title_source -> ParserSourceType,
        #[max_length = 255]
        anime_title_value -> Varchar,
        episode_no_source -> ParserSourceType,
        #[max_length = 50]
        episode_no_value -> Varchar,
        series_no_source -> Nullable<ParserSourceType>,
        #[max_length = 50]
        series_no_value -> Nullable<Varchar>,
        subtitle_group_source -> Nullable<ParserSourceType>,
        #[max_length = 255]
        subtitle_group_value -> Nullable<Varchar>,
        resolution_source -> Nullable<ParserSourceType>,
        #[max_length = 50]
        resolution_value -> Nullable<Varchar>,
        season_source -> Nullable<ParserSourceType>,
        #[max_length = 20]
        season_value -> Nullable<Varchar>,
        year_source -> Nullable<ParserSourceType>,
        #[max_length = 10]
        year_value -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(anime_links -> anime_series (series_id));
diesel::joinable!(anime_links -> raw_anime_items (raw_item_id));
diesel::joinable!(anime_links -> subtitle_groups (group_id));
diesel::joinable!(anime_series -> animes (anime_id));
diesel::joinable!(anime_series -> seasons (season_id));
diesel::joinable!(downloader_capabilities -> service_modules (module_id));
diesel::joinable!(downloads -> anime_links (link_id));
diesel::joinable!(downloads -> service_modules (module_id));
diesel::joinable!(raw_anime_items -> subscriptions (subscription_id));
diesel::joinable!(raw_anime_items -> title_parsers (parser_id));
diesel::joinable!(subscription_conflicts -> subscriptions (subscription_id));

diesel::allow_tables_to_appear_in_same_query!(
    anime_links,
    anime_series,
    animes,
    cron_logs,
    downloader_capabilities,
    downloads,
    filter_rules,
    raw_anime_items,
    seasons,
    service_modules,
    subscription_conflicts,
    subscriptions,
    subtitle_groups,
    title_parsers,
);
