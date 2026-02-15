import { Schema } from "effect"

export const PreviewItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
})
export type PreviewItem = typeof PreviewItem.Type

export const RawPreviewItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
  status: Schema.String,
})
export type RawPreviewItem = typeof RawPreviewItem.Type
