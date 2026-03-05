import { Effect, Context } from "effect"
import type { AggregatedSearchResponse, DetailResponse } from "@/schemas/search"
import type { AnimeWork, Anime, Season, SubtitleGroup, AnimeLink, AnimeRich, AnimeLinkRich, AnimeCoverImage, ConflictingLink } from "@/schemas/anime"
import type { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import type { TitleParser, ParserPreviewResponse, ParserWithReparseResponse, DeleteWithReparseResponse } from "@/schemas/parser"
import type { Subscription } from "@/schemas/subscription"
import type { ServiceModule } from "@/schemas/service-module"
import type { RawAnimeItem, DownloadRow } from "@/schemas/download"
import type { DashboardStats } from "@/schemas/dashboard"
import type { AiSettings, AiPromptSettings, PendingAiResult, ConfirmPendingRequest, RegenerateRequest } from "@/schemas/ai"

export class CoreApi extends Context.Tag("CoreApi")<
  CoreApi,
  {
    readonly getAnimeWorks: Effect.Effect<readonly AnimeWork[]>
    readonly createAnimeWork: (title: string) => Effect.Effect<AnimeWork>
    readonly deleteAnimeWork: (id: number) => Effect.Effect<void>
    readonly getSubscriptions: Effect.Effect<readonly Subscription[]>
    readonly getFilterRules: (targetType?: string, targetId?: number) => Effect.Effect<readonly FilterRule[]>
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
      search?: string
    }) => Effect.Effect<readonly RawAnimeItem[]>
    readonly getDownloads: (params: {
      status?: string
      limit?: number
      offset?: number
    }) => Effect.Effect<readonly DownloadRow[]>
    readonly getHealth: Effect.Effect<{ status: string; service: string }>
    readonly getSubtitleGroups: Effect.Effect<readonly SubtitleGroup[]>
    readonly createSubtitleGroup: (name: string) => Effect.Effect<SubtitleGroup>
    readonly deleteSubtitleGroup: (groupId: number) => Effect.Effect<void>
    readonly getAnime: (animeWorkId: number) => Effect.Effect<readonly Anime[]>
    readonly getOneAnime: (animeId: number) => Effect.Effect<Anime>
    readonly createAnime: (req: {
      anime_id: number; series_no: number; season_id: number;
      description?: string; aired_date?: string; end_date?: string;
    }) => Effect.Effect<Anime>
    readonly getSeasons: Effect.Effect<readonly Season[]>
    readonly createSeason: (req: { year: number; season: string }) => Effect.Effect<Season>
    readonly getAnimeLinks: (animeId: number) => Effect.Effect<readonly AnimeLink[]>
    readonly getAllAnime: (params?: { excludeEmpty?: boolean }) => Effect.Effect<readonly AnimeRich[]>
    readonly getDashboardStats: Effect.Effect<DashboardStats>
    readonly getAnimeLinksRich: (animeId: number) => Effect.Effect<readonly AnimeLinkRich[]>
    readonly updateAnime: (animeId: number, req: {
      season_id?: number | null
      description?: string | null
      aired_date?: string | null
      end_date?: string | null
    }) => Effect.Effect<Anime>
    readonly getRawItem: (itemId: number) => Effect.Effect<RawAnimeItem>
    readonly getDownloaderModules: Effect.Effect<readonly ServiceModule[]>
    readonly getFetcherModules: Effect.Effect<readonly ServiceModule[]>
    readonly updateServiceModule: (id: number, req: { priority?: number; is_enabled?: boolean }) => Effect.Effect<ServiceModule>
    readonly createSubscription: (req: {
      source_url: string
      name?: string
      fetch_interval_minutes?: number
      preferred_downloader_id?: number | null
      fetcher_id?: number
    }) => Effect.Effect<Subscription>
    readonly updateSubscription: (id: number, req: { name?: string; fetch_interval_minutes?: number; is_active?: boolean; preferred_downloader_id?: number | null }) => Effect.Effect<Subscription>
    readonly deleteSubscription: (id: number, purge?: boolean) => Effect.Effect<void>
    readonly triggerFetch: (subscriptionId: number) => Effect.Effect<void>
    readonly getRawItemsCount: (subscriptionId: number, status: string) => Effect.Effect<number>
    readonly getAnimeCoverImages: (
      animeWorkId: number,
    ) => Effect.Effect<readonly AnimeCoverImage[]>
    readonly setDefaultCoverImage: (
      animeWorkId: number,
      coverId: number,
    ) => Effect.Effect<void>
    readonly getConflictingLinks: Effect.Effect<readonly ConflictingLink[]>
    readonly getAnimeWorksFiltered: (params?: { hasLinks?: boolean }) => Effect.Effect<readonly AnimeWork[]>
    readonly search: (query: string) => Effect.Effect<AggregatedSearchResponse>
    readonly getDetail: (detail_key: string, source: string) => Effect.Effect<DetailResponse>
    // AI 設定
    readonly getAiSettings: Effect.Effect<AiSettings>
    readonly updateAiSettings: (req: Partial<Pick<AiSettings, "base_url" | "api_key" | "model_name">>) => Effect.Effect<void>
    readonly testAiConnection: Effect.Effect<{ ok: boolean; error?: string }>
    readonly getAiPromptSettings: Effect.Effect<AiPromptSettings>
    readonly updateAiPromptSettings: (req: Partial<Omit<AiPromptSettings, "id" | "created_at" | "updated_at">>) => Effect.Effect<void>
    readonly revertParserPrompt: Effect.Effect<{ value: string }>
    readonly revertFilterPrompt: Effect.Effect<{ value: string }>
    // 待確認管理
    readonly getPendingAiResults: (params?: { result_type?: string; status?: string; subscription_id?: number }) => Effect.Effect<readonly PendingAiResult[]>
    readonly getPendingAiResult: (id: number) => Effect.Effect<PendingAiResult>
    readonly updatePendingAiResult: (id: number, generated_data: Record<string, unknown>) => Effect.Effect<PendingAiResult>
    readonly confirmPendingAiResult: (id: number, req: ConfirmPendingRequest) => Effect.Effect<void>
    readonly rejectPendingAiResult: (id: number) => Effect.Effect<void>
    readonly regeneratePendingAiResult: (id: number, req: RegenerateRequest) => Effect.Effect<PendingAiResult>
  }
>() {}
