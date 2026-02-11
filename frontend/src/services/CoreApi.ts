import { Effect, Context } from "effect"
import type { Anime, AnimeSeries, Season, SubtitleGroup, AnimeLink } from "@/schemas/anime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import type { TitleParser, ParserPreviewResponse } from "@/schemas/parser"
import type { Subscription } from "@/schemas/subscription"
import type { RawAnimeItem, DownloadRow } from "@/schemas/download"

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
    readonly createParser: (req: Record<string, unknown>) => Effect.Effect<TitleParser>
    readonly deleteParser: (id: number) => Effect.Effect<void>
    readonly previewParser: (req: Record<string, unknown>) => Effect.Effect<ParserPreviewResponse>
    readonly getRawItems: (params: {
      status?: string
      subscription_id?: number
      limit?: number
      offset?: number
    }) => Effect.Effect<readonly RawAnimeItem[]>
    readonly getDownloads: (params: {
      status?: string
      limit?: number
      offset?: number
    }) => Effect.Effect<readonly DownloadRow[]>
    readonly getConflicts: Effect.Effect<readonly Record<string, unknown>[]>
    readonly resolveConflict: (conflictId: number, fetcherId: number) => Effect.Effect<unknown>
    readonly getHealth: Effect.Effect<{ status: string; service: string }>
    readonly getSubtitleGroups: Effect.Effect<readonly SubtitleGroup[]>
    readonly createSubtitleGroup: (name: string) => Effect.Effect<SubtitleGroup>
    readonly deleteSubtitleGroup: (groupId: number) => Effect.Effect<void>
    readonly getAnimeSeries: (animeId: number) => Effect.Effect<readonly AnimeSeries[]>
    readonly getOneAnimeSeries: (seriesId: number) => Effect.Effect<AnimeSeries>
    readonly createAnimeSeries: (req: {
      anime_id: number; series_no: number; season_id: number;
      description?: string; aired_date?: string; end_date?: string;
    }) => Effect.Effect<AnimeSeries>
    readonly getSeasons: Effect.Effect<readonly Season[]>
    readonly createSeason: (req: { year: number; season: string }) => Effect.Effect<Season>
    readonly getAnimeLinks: (seriesId: number) => Effect.Effect<readonly AnimeLink[]>
  }
>() {}
