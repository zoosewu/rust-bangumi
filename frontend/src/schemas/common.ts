import { Schema } from "effect"

export const PreviewItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
})
export type PreviewItem = typeof PreviewItem.Type
