import { Schema } from "effect"

export const Anime = Schema.Struct({
  anime_id: Schema.Number,
  title: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type Anime = typeof Anime.Type

export const AnimeSeries = Schema.Struct({
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
export type AnimeSeries = typeof AnimeSeries.Type

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
