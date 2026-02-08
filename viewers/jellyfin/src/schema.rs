// @generated automatically by Diesel CLI.

diesel::table! {
    bangumi_subjects (bangumi_id) {
        bangumi_id -> Int4,
        title -> Text,
        title_cn -> Nullable<Text>,
        summary -> Nullable<Text>,
        rating -> Nullable<Float4>,
        cover_url -> Nullable<Text>,
        air_date -> Nullable<Date>,
        episode_count -> Nullable<Int4>,
        raw_json -> Nullable<Jsonb>,
        fetched_at -> Timestamp,
    }
}

diesel::table! {
    bangumi_episodes (bangumi_ep_id) {
        bangumi_ep_id -> Int4,
        bangumi_id -> Int4,
        episode_no -> Int4,
        title -> Nullable<Text>,
        title_cn -> Nullable<Text>,
        air_date -> Nullable<Date>,
        summary -> Nullable<Text>,
        fetched_at -> Timestamp,
    }
}

diesel::table! {
    bangumi_mapping (core_series_id) {
        core_series_id -> Int4,
        bangumi_id -> Int4,
        title_cache -> Nullable<Text>,
        #[max_length = 20]
        source -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    sync_tasks (task_id) {
        task_id -> Int4,
        download_id -> Int4,
        core_series_id -> Int4,
        episode_no -> Int4,
        source_path -> Text,
        target_path -> Nullable<Text>,
        #[max_length = 20]
        status -> Varchar,
        error_message -> Nullable<Text>,
        created_at -> Timestamp,
        completed_at -> Nullable<Timestamp>,
    }
}

diesel::joinable!(bangumi_episodes -> bangumi_subjects (bangumi_id));
diesel::joinable!(bangumi_mapping -> bangumi_subjects (bangumi_id));

diesel::allow_tables_to_appear_in_same_query!(
    bangumi_subjects,
    bangumi_episodes,
    bangumi_mapping,
    sync_tasks,
);
