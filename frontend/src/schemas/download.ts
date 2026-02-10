import { Schema } from "effect"

export const RawAnimeItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
  description: Schema.NullOr(Schema.String),
  download_url: Schema.String,
  pub_date: Schema.NullOr(Schema.String),
  subscription_id: Schema.Number,
  status: Schema.String,
  parser_id: Schema.NullOr(Schema.Number),
  error_message: Schema.NullOr(Schema.String),
  parsed_at: Schema.NullOr(Schema.String),
  created_at: Schema.String,
})
export type RawAnimeItem = typeof RawAnimeItem.Type
