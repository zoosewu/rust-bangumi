import { Schema } from "effect"

export const ServiceModule = Schema.Struct({
  module_id: Schema.Number,
  name: Schema.String,
  module_type: Schema.String,
  priority: Schema.Number,
  is_enabled: Schema.Boolean,
  base_url: Schema.String,
  description: Schema.NullOr(Schema.String),
  updated_at: Schema.String,
})
export type ServiceModule = typeof ServiceModule.Type
