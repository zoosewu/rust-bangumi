import { Schema } from "effect"

export const TitleParser = Schema.Struct({
  parser_id: Schema.Number,
  name: Schema.String,
  description: Schema.NullOr(Schema.String),
  priority: Schema.Number,
  is_enabled: Schema.Boolean,
  condition_regex: Schema.String,
  parse_regex: Schema.String,
  anime_title_source: Schema.String,
  anime_title_value: Schema.String,
  episode_no_source: Schema.String,
  episode_no_value: Schema.String,
  series_no_source: Schema.NullOr(Schema.String),
  series_no_value: Schema.NullOr(Schema.String),
  subtitle_group_source: Schema.NullOr(Schema.String),
  subtitle_group_value: Schema.NullOr(Schema.String),
  resolution_source: Schema.NullOr(Schema.String),
  resolution_value: Schema.NullOr(Schema.String),
  season_source: Schema.NullOr(Schema.String),
  season_value: Schema.NullOr(Schema.String),
  year_source: Schema.NullOr(Schema.String),
  year_value: Schema.NullOr(Schema.String),
  created_from_type: Schema.NullOr(Schema.String),
  created_from_id: Schema.NullOr(Schema.Number),
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type TitleParser = typeof TitleParser.Type

export const ParsedFields = Schema.Struct({
  anime_title: Schema.String,
  episode_no: Schema.Number,
  series_no: Schema.Number,
  subtitle_group: Schema.NullOr(Schema.String),
  resolution: Schema.NullOr(Schema.String),
  season: Schema.NullOr(Schema.String),
  year: Schema.NullOr(Schema.String),
})

export const ParserPreviewResult = Schema.Struct({
  title: Schema.String,
  before_matched_by: Schema.NullOr(Schema.String),
  after_matched_by: Schema.NullOr(Schema.String),
  is_newly_matched: Schema.Boolean,
  is_override: Schema.Boolean,
  parse_result: Schema.NullOr(ParsedFields),
  parse_error: Schema.optionalWith(Schema.NullOr(Schema.String), { default: () => null }),
})

export const ParserPreviewResponse = Schema.Struct({
  condition_regex_valid: Schema.Boolean,
  parse_regex_valid: Schema.Boolean,
  regex_error: Schema.NullOr(Schema.String),
  results: Schema.Array(ParserPreviewResult),
})
export type ParserPreviewResponse = typeof ParserPreviewResponse.Type
