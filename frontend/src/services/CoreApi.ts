import { Effect, Context } from "effect"
import type { Anime } from "@/schemas/anime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import type { TitleParser, ParserPreviewResponse } from "@/schemas/parser"
import type { Subscription } from "@/schemas/subscription"
import type { RawAnimeItem } from "@/schemas/download"

export class CoreApi extends Context.Tag("CoreApi")<
  CoreApi,
  {
    readonly getAnimes: Effect.Effect<readonly Anime[]>
    readonly createAnime: (title: string) => Effect.Effect<Anime>
    readonly deleteAnime: (id: number) => Effect.Effect<void>
    readonly getSubscriptions: Effect.Effect<readonly Subscription[]>
    readonly getFilterRules: (targetType: string, targetId?: number) => Effect.Effect<readonly FilterRule[]>
    readonly createFilterRule: (req: {
      target_type: string
      target_id?: number
      rule_order: number
      is_positive: boolean
      regex_pattern: string
    }) => Effect.Effect<FilterRule>
    readonly deleteFilterRule: (ruleId: number) => Effect.Effect<void>
    readonly previewFilter: (req: {
      regex_pattern: string
      is_positive: boolean
      subscription_id?: number
      exclude_filter_id?: number
      limit?: number
    }) => Effect.Effect<FilterPreviewResponse>
    readonly getParsers: Effect.Effect<readonly TitleParser[]>
    readonly previewParser: (req: Record<string, unknown>) => Effect.Effect<ParserPreviewResponse>
    readonly getRawItems: (params: {
      status?: string
      subscription_id?: number
      limit?: number
      offset?: number
    }) => Effect.Effect<readonly RawAnimeItem[]>
    readonly getHealth: Effect.Effect<{ status: string; service: string }>
  }
>() {}
