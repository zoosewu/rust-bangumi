import { Schema } from "effect"

export const ServiceStatus = Schema.Struct({
  name: Schema.String,
  module_type: Schema.String,
  is_healthy: Schema.Boolean,
})

export const DashboardStats = Schema.Struct({
  total_anime: Schema.Number,
  total_series: Schema.Number,
  active_subscriptions: Schema.Number,
  total_downloads: Schema.Number,
  downloading: Schema.Number,
  completed: Schema.Number,
  failed: Schema.Number,
  pending_raw_items: Schema.Number,
  pending_conflicts: Schema.Number,
  services: Schema.Array(ServiceStatus),
})
export type DashboardStats = typeof DashboardStats.Type
