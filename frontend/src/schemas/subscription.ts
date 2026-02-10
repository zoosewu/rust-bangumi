import { Schema } from "effect"

export const Subscription = Schema.Struct({
  subscription_id: Schema.Number,
  fetcher_id: Schema.Number,
  source_url: Schema.String,
  name: Schema.NullOr(Schema.String),
  description: Schema.NullOr(Schema.String),
  last_fetched_at: Schema.NullOr(Schema.String),
  next_fetch_at: Schema.NullOr(Schema.String),
  fetch_interval_minutes: Schema.Number,
  is_active: Schema.Boolean,
  created_at: Schema.String,
  updated_at: Schema.String,
})
export type Subscription = typeof Subscription.Type
