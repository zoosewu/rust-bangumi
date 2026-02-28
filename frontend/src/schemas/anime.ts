import { Schema } from "effect"

export const AnimeWork = Schema.Struct({
  anime_id: Schema.Number,
  title: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type AnimeWork = typeof AnimeWork.Type

export const Anime = Schema.Struct({
  series_id: Schema.Number,
  anime_id: Schema.Number,
  series_no: Schema.Number,
  season_id: Schema.Number,
  description: Schema.NullOr(Schema.String),
  aired_date: Schema.NullOr(Schema.String),
  end_date: Schema.NullOr(Schema.String),
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type Anime = typeof Anime.Type

export const Season = Schema.Struct({
  season_id: Schema.Number,
  year: Schema.Number,
  season: Schema.String,
  created_at: Schema.String,
})
export type Season = typeof Season.Type

export const SubtitleGroup = Schema.Struct({
  group_id: Schema.Number,
  group_name: Schema.String,
  created_at: Schema.String,
})
export type SubtitleGroup = typeof SubtitleGroup.Type

export const AnimeLink = Schema.Struct({
  link_id: Schema.Number,
  series_id: Schema.Number,
  group_id: Schema.Number,
  episode_no: Schema.Number,
  title: Schema.NullOr(Schema.String),
  url: Schema.String,
  source_hash: Schema.String,
  created_at: Schema.String,
})
export type AnimeLink = typeof AnimeLink.Type

export const SeasonInfo = Schema.Struct({
  year: Schema.Number,
  season: Schema.String,
})

export const SubscriptionInfo = Schema.Struct({
  subscription_id: Schema.Number,
  name: Schema.NullOr(Schema.String),
})

export const AnimeRich = Schema.Struct({
  series_id: Schema.Number,
  anime_id: Schema.Number,
  anime_title: Schema.String,
  series_no: Schema.Number,
  season: SeasonInfo,
  episode_downloaded: Schema.Number,
  episode_found: Schema.Number,
  subscriptions: Schema.Array(SubscriptionInfo),
  description: Schema.NullOr(Schema.String),
  aired_date: Schema.NullOr(Schema.String),
  end_date: Schema.NullOr(Schema.String),
  created_at: Schema.String,
  updated_at: Schema.String,
  cover_image_url: Schema.NullOr(Schema.String),
})
export type AnimeRich = typeof AnimeRich.Type

export const DownloadInfo = Schema.Struct({
  download_id: Schema.Number,
  status: Schema.String,
  progress: Schema.NullOr(Schema.Number),
  torrent_hash: Schema.NullOr(Schema.String),
})

export const AnimeLinkRich = Schema.Struct({
  link_id: Schema.Number,
  series_id: Schema.Number,
  group_id: Schema.Number,
  group_name: Schema.String,
  episode_no: Schema.Number,
  title: Schema.NullOr(Schema.String),
  url: Schema.String,
  source_hash: Schema.String,
  filtered_flag: Schema.Boolean,
  conflict_flag: Schema.Boolean,
  conflicting_link_ids: Schema.Array(Schema.Number),
  download: Schema.NullOr(DownloadInfo),
  created_at: Schema.String,
})
export type AnimeLinkRich = typeof AnimeLinkRich.Type

export const AnimeCoverImage = Schema.Struct({
  cover_id: Schema.Number,
  anime_id: Schema.Number,
  image_url: Schema.String,
  service_module_id: Schema.NullOr(Schema.Number),
  source_name: Schema.String,
  is_default: Schema.Boolean,
  created_at: Schema.String,
})
export type AnimeCoverImage = typeof AnimeCoverImage.Type

export const ConflictingLink = Schema.Struct({
  link_id: Schema.Number,
  episode_no: Schema.Number,
  group_name: Schema.String,
  url: Schema.String,
  conflicting_link_ids: Schema.Array(Schema.Number),
  series_id: Schema.Number,
  series_no: Schema.Number,
  anime_work_id: Schema.Number,
  anime_work_title: Schema.String,
  subscription_id: Schema.NullOr(Schema.Number),
  subscription_name: Schema.NullOr(Schema.String),
})
export type ConflictingLink = typeof ConflictingLink.Type
