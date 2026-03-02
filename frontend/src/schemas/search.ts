import { Schema } from "effect"

export const SearchResultSchema = Schema.Struct({
  title: Schema.String,
  description: Schema.NullOr(Schema.String),
  thumbnail_url: Schema.NullOr(Schema.String),
  subscription_url: Schema.String,
  source: Schema.String,
})

export const AggregatedSearchResponseSchema = Schema.Struct({
  results: Schema.Array(SearchResultSchema),
})

export type SearchResult = typeof SearchResultSchema.Type
export type AggregatedSearchResponse = typeof AggregatedSearchResponseSchema.Type
