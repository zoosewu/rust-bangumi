import { Schema } from "effect"
import { PreviewItem, RawPreviewItem } from "./common"

export const FilterRule = Schema.Struct({
  rule_id: Schema.Number,
  target_type: Schema.String,
  target_id: Schema.NullOr(Schema.Number),
  rule_order: Schema.Number,
  is_positive: Schema.Boolean,
  regex_pattern: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type FilterRule = typeof FilterRule.Type

export const FilterPreviewPanel = Schema.Struct({
  passed_items: Schema.Array(PreviewItem),
  filtered_items: Schema.Array(PreviewItem),
})

export const FilterPreviewResponse = Schema.Struct({
  regex_valid: Schema.Boolean,
  regex_error: Schema.NullOr(Schema.String),
  before: FilterPreviewPanel,
  after: FilterPreviewPanel,
})
export type FilterPreviewResponse = typeof FilterPreviewResponse.Type

export const RawFilterPreviewPanel = Schema.Struct({
  passed_items: Schema.Array(RawPreviewItem),
  filtered_items: Schema.Array(RawPreviewItem),
})

export const RawFilterPreviewResponse = Schema.Struct({
  regex_valid: Schema.Boolean,
  regex_error: Schema.NullOr(Schema.String),
  before: RawFilterPreviewPanel,
  after: RawFilterPreviewPanel,
})
export type RawFilterPreviewResponse = typeof RawFilterPreviewResponse.Type
