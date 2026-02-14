import { Effect, Context } from "effect"
import type { Anime, AnimeSeries, Season, SubtitleGroup, AnimeLink, AnimeSeriesRich, AnimeLinkRich } from "@/schemas/anime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import type { TitleParser, ParserPreviewResponse, ParserWithReparseResponse, DeleteWithReparseResponse } from "@/schemas/parser"
import type { Subscription } from "@/schemas/subscription"
import type { RawAnimeItem, DownloadRow } from "@/schemas/download"
import type { DashboardStats } from "@/schemas/dashboard"

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
      target_type: string
      target_id?: number | null
      regex_pattern: string
      is_positive: boolean
      exclude_filter_id?: number
    }) => Effect.Effect<FilterPreviewResponse>
    readonly getParsers: (params?: {
      created_from_type?: string
      created_from_id?: number
    }) => Effect.Effect<readonly TitleParser[]>
    readonly createParser: (req: Record<string, unknown>) => Effect.Effect<ParserWithReparseResponse>
    readonly updateParser: (id: number, req: Record<string, unknown>) => Effect.Effect<ParserWithReparseResponse>
    readonly deleteParser: (id: number) => Effect.Effect<DeleteWithReparseResponse>
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
    readonly getAllAnimeSeries: Effect.Effect<readonly AnimeSeriesRich[]>
    readonly getDashboardStats: Effect.Effect<DashboardStats>
    readonly getAnimeLinksRich: (seriesId: number) => Effect.Effect<readonly AnimeLinkRich[]>
    readonly updateAnimeSeries: (seriesId: number, req: {
      season_id?: number | null
      description?: string | null
      aired_date?: string | null
      end_date?: string | null
    }) => Effect.Effect<AnimeSeries>
    readonly getRawItem: (itemId: number) => Effect.Effect<RawAnimeItem>
    readonly createSubscription: (req: {
      source_url: string
      name?: string
      fetch_interval_minutes?: number
    }) => Effect.Effect<Subscription>
    readonly deleteSubscription: (id: number) => Effect.Effect<void>
    readonly getRawItemsCount: (subscriptionId: number, status: string) => Effect.Effect<number>
  }
>() {}
