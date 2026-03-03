import { Schema } from "effect"

export const SearchResultSchema = Schema.Struct({
  title: Schema.String,
  thumbnail_url: Schema.NullOr(Schema.String),
  detail_key: Schema.String,
  source: Schema.String,
})

export const AggregatedSearchResponseSchema = Schema.Struct({
  results: Schema.Array(SearchResultSchema),
})

export type SearchResult = typeof SearchResultSchema.Type
export type AggregatedSearchResponse = typeof AggregatedSearchResponseSchema.Type

export const DetailItemSchema = Schema.Struct({
  subgroup_name: Schema.String,
  rss_url: Schema.String,
})

export const DetailResponseSchema = Schema.Struct({
  items: Schema.Array(DetailItemSchema),
})

export type DetailItem = typeof DetailItemSchema.Type
export type DetailResponse = typeof DetailResponseSchema.Type
