import { Schema } from "effect"

export const RawItemDownloadInfo = Schema.Struct({
  status: Schema.String,
  progress: Schema.NullOr(Schema.Number),
})

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
  download: Schema.optionalWith(Schema.NullOr(RawItemDownloadInfo), { default: () => null }),
  filter_passed: Schema.optionalWith(Schema.NullOr(Schema.Boolean), { default: () => null }),
})
export type RawAnimeItem = typeof RawAnimeItem.Type

export const DownloadRow = Schema.Struct({
  download_id: Schema.Number,
  link_id: Schema.Number,
  title: Schema.NullOr(Schema.String),
  downloader_type: Schema.String,
  status: Schema.String,
  progress: Schema.NullOr(Schema.Number),
  downloaded_bytes: Schema.NullOr(Schema.Number),
  total_bytes: Schema.NullOr(Schema.Number),
  error_message: Schema.NullOr(Schema.String),
  torrent_hash: Schema.NullOr(Schema.String),
  file_path: Schema.NullOr(Schema.String),
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type DownloadRow = typeof DownloadRow.Type
