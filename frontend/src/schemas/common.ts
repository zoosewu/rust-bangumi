import { Schema } from "effect"

export const PreviewItem = Schema.Struct({
  item_id: Schema.Number,
  title: Schema.String,
  conflict_flag: Schema.Boolean,
  anime_title: Schema.NullOr(Schema.String),
  series_no: Schema.NullOr(Schema.Number),
  episode_no: Schema.Number,
  group_name: Schema.NullOr(Schema.String),
})
export type PreviewItem = typeof PreviewItem.Type
