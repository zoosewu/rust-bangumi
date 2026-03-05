import { Effect, Layer, Schema } from "effect"
import * as HttpClient from "@effect/platform/HttpClient"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"
import { CoreApi } from "@/services/CoreApi"
import {
  AggregatedSearchResponseSchema,
  DetailResponseSchema,
} from "@/schemas/search"
import { AnimeWork, Anime, Season, SubtitleGroup, AnimeLink, AnimeRich, AnimeLinkRich, AnimeCoverImage, ConflictingLink } from "@/schemas/anime"
import { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import { TitleParser, ParserPreviewResponse, ParserWithReparseResponse, DeleteWithReparseResponse } from "@/schemas/parser"
import { Subscription } from "@/schemas/subscription"
import { ServiceModule } from "@/schemas/service-module"
import { RawAnimeItem, DownloadRow } from "@/schemas/download"
import { DashboardStats } from "@/schemas/dashboard"
import type { AiSettings, AiPromptSettings, PendingAiResult, ConfirmPendingRequest, RegenerateRequest } from "@/schemas/ai"

const makeCoreApi = Effect.gen(function* () {
  const client = yield* HttpClient.HttpClient

  const fetchJson = <A, I>(
    request: HttpClientRequest.HttpClientRequest,
    schema: Schema.Schema<A, I>,
  ) =>
    client.execute(request).pipe(
      Effect.flatMap((response) => response.json),
      Effect.flatMap(Schema.decodeUnknown(schema)),
      Effect.scoped,
      Effect.orDie,
    )

  const postJson = <A, I>(url: string, body: unknown, schema: Schema.Schema<A, I>) =>
    client
      .execute(
        HttpClientRequest.post(url).pipe(HttpClientRequest.bodyUnsafeJson(body)),
      )
      .pipe(
        Effect.flatMap((response) => response.json),
        Effect.flatMap(Schema.decodeUnknown(schema)),
        Effect.scoped,
        Effect.orDie,
      )

  return CoreApi.of({
    getAnimeWorks: fetchJson(
      HttpClientRequest.get("/api/core/anime-works"),
      Schema.Struct({ animes: Schema.Array(AnimeWork) }),
    ).pipe(Effect.map((r) => r.animes)),

    createAnimeWork: (title) => postJson("/api/core/anime-works", { title }, AnimeWork),

    deleteAnimeWork: (id) =>
      client
        .execute(HttpClientRequest.del(`/api/core/anime-works/${id}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getSubscriptions: fetchJson(
      HttpClientRequest.get("/api/core/subscriptions"),
      Schema.Struct({ subscriptions: Schema.Array(Subscription) }),
    ).pipe(Effect.map((r) => r.subscriptions)),

    getFilterRules: (targetType, targetId) => {
      const qs = new URLSearchParams()
      if (targetType) qs.set("target_type", targetType)
      if (targetId != null) qs.set("target_id", String(targetId))
      const q = qs.toString()
      return fetchJson(
        HttpClientRequest.get(`/api/core/filters${q ? `?${q}` : ""}`),
        Schema.Struct({ rules: Schema.Array(FilterRule) }),
      ).pipe(Effect.map((r) => r.rules))
    },

    createFilterRule: (req) =>
      postJson("/api/core/filters", req, Schema.Any).pipe(
        Effect.map((r) => r as typeof FilterRule.Type),
      ),

    deleteFilterRule: (ruleId) =>
      client
        .execute(HttpClientRequest.del(`/api/core/filters/${ruleId}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    previewFilter: (req) =>
      postJson("/api/core/filters/preview", req, FilterPreviewResponse),

    getParsers: (params) => {
      const qs = new URLSearchParams()
      if (params?.created_from_type) qs.set("created_from_type", params.created_from_type)
      if (params?.created_from_id != null) qs.set("created_from_id", String(params.created_from_id))
      const query = qs.toString()
      return fetchJson(
        HttpClientRequest.get(`/api/core/parsers${query ? `?${query}` : ""}`),
        Schema.Array(TitleParser),
      )
    },

    createParser: (req) =>
      postJson("/api/core/parsers", req, ParserWithReparseResponse),

    updateParser: (id, req) =>
      client
        .execute(
          HttpClientRequest.put(`/api/core/parsers/${id}`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((response) => response.json),
          Effect.flatMap(Schema.decodeUnknown(ParserWithReparseResponse)),
          Effect.scoped,
          Effect.orDie,
        ),

    deleteParser: (id) =>
      client
        .execute(HttpClientRequest.del(`/api/core/parsers/${id}`))
        .pipe(
          Effect.flatMap((response) => response.json),
          Effect.flatMap(Schema.decodeUnknown(DeleteWithReparseResponse)),
          Effect.scoped,
          Effect.orDie,
        ),

    previewParser: (req) =>
      postJson("/api/core/parsers/preview", req, ParserPreviewResponse),

    getRawItems: (params) => {
      const qs = new URLSearchParams()
      if (params.status) qs.set("status", params.status)
      if (params.subscription_id != null)
        qs.set("subscription_id", String(params.subscription_id))
      if (params.limit != null) qs.set("limit", String(params.limit))
      if (params.offset != null) qs.set("offset", String(params.offset))
      if (params.search) qs.set("search", params.search)
      return fetchJson(
        HttpClientRequest.get(`/api/core/raw-items?${qs.toString()}`),
        Schema.Array(RawAnimeItem),
      )
    },

    getDownloads: (params) => {
      const qs = new URLSearchParams()
      if (params.status) qs.set("status", params.status)
      if (params.limit != null) qs.set("limit", String(params.limit))
      if (params.offset != null) qs.set("offset", String(params.offset))
      return fetchJson(
        HttpClientRequest.get(`/api/core/downloads?${qs.toString()}`),
        Schema.Array(DownloadRow),
      )
    },

    getHealth: fetchJson(
      HttpClientRequest.get("/api/core/health"),
      Schema.Struct({ status: Schema.String, service: Schema.String }),
    ),

    getSubtitleGroups: fetchJson(
      HttpClientRequest.get("/api/core/subtitle-groups"),
      Schema.Struct({ groups: Schema.Array(SubtitleGroup) }),
    ).pipe(Effect.map((r) => r.groups)),

    createSubtitleGroup: (name) =>
      postJson("/api/core/subtitle-groups", { group_name: name }, SubtitleGroup),

    deleteSubtitleGroup: (groupId) =>
      client
        .execute(HttpClientRequest.del(`/api/core/subtitle-groups/${groupId}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getAnime: (animeWorkId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/anime-works/${animeWorkId}/anime`),
        Schema.Struct({ series: Schema.Array(Anime) }),
      ).pipe(Effect.map((r) => r.series)),

    getOneAnime: (animeId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/anime/${animeId}`),
        Anime,
      ),

    createAnime: (req) =>
      postJson("/api/core/anime", req, Anime),

    getSeasons: fetchJson(
      HttpClientRequest.get("/api/core/seasons"),
      Schema.Struct({ seasons: Schema.Array(Season) }),
    ).pipe(Effect.map((r) => r.seasons)),

    createSeason: (req) =>
      postJson("/api/core/seasons", req, Season),

    getAnimeLinks: (animeId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/links/${animeId}`),
        Schema.Struct({ links: Schema.Array(AnimeLink) }),
      ).pipe(Effect.map((r) => r.links)),

    getAllAnime: (params) => {
      const qs = new URLSearchParams()
      if (params?.excludeEmpty) qs.set("exclude_empty", "true")
      const query = qs.toString()
      return fetchJson(
        HttpClientRequest.get(`/api/core/anime${query ? `?${query}` : ""}`),
        Schema.Struct({ series: Schema.Array(AnimeRich) }),
      ).pipe(Effect.map((r) => r.series))
    },

    getDashboardStats: fetchJson(
      HttpClientRequest.get("/api/core/dashboard/stats"),
      DashboardStats,
    ),

    getAnimeLinksRich: (animeId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/links/${animeId}`),
        Schema.Struct({ links: Schema.Array(AnimeLinkRich) }),
      ).pipe(Effect.map((r) => r.links)),

    updateAnime: (animeId, req) =>
      client
        .execute(
          HttpClientRequest.put(`/api/core/anime/${animeId}`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((response) => response.json),
          Effect.flatMap(Schema.decodeUnknown(Anime)),
          Effect.scoped,
          Effect.orDie,
        ),

    getRawItem: (itemId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/raw-items/${itemId}`),
        RawAnimeItem,
      ),

    getDownloaderModules: fetchJson(
      HttpClientRequest.get("/api/core/services/downloader-modules"),
      Schema.Struct({ modules: Schema.Array(ServiceModule) }),
    ).pipe(Effect.map((r) => r.modules)),

    getFetcherModules: fetchJson(
      HttpClientRequest.get("/api/core/fetcher-modules"),
      Schema.Struct({ modules: Schema.Array(ServiceModule) }),
    ).pipe(Effect.map((r) => r.modules)),

    updateServiceModule: (id, req) =>
      client
        .execute(
          HttpClientRequest.patch(`/api/core/services/${id}/update`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((r) => r.json),
          Effect.flatMap(Schema.decodeUnknown(ServiceModule)),
          Effect.scoped,
          Effect.orDie,
        ),

    createSubscription: (req) =>
      postJson("/api/core/subscriptions", req, Subscription),

    updateSubscription: (id, req) =>
      client
        .execute(
          HttpClientRequest.patch(`/api/core/subscriptions/${id}`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((r) => r.json),
          Effect.flatMap(Schema.decodeUnknown(Subscription)),
          Effect.scoped,
          Effect.orDie,
        ),

    deleteSubscription: (id, purge) =>
      client
        .execute(HttpClientRequest.del(`/api/core/subscriptions/${id}${purge ? '?purge=true' : ''}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    triggerFetch: (subscriptionId) =>
      client
        .execute(HttpClientRequest.post(`/api/core/subscriptions/${subscriptionId}/fetch`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getRawItemsCount: (subscriptionId, status) =>
      fetchJson(
        HttpClientRequest.get(
          `/api/core/raw-items/count?subscription_id=${subscriptionId}&status=${status}`,
        ),
        Schema.Struct({ count: Schema.Number }),
      ).pipe(Effect.map((r) => r.count)),

    getAnimeCoverImages: (animeWorkId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/anime-works/${animeWorkId}/covers`),
        Schema.Array(AnimeCoverImage),
      ),

    setDefaultCoverImage: (animeWorkId, coverId) =>
      client
        .execute(
          HttpClientRequest.post(`/api/core/anime-works/${animeWorkId}/covers/${coverId}/set-default`).pipe(
            HttpClientRequest.bodyUnsafeJson({}),
          ),
        )
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getConflictingLinks: fetchJson(
      HttpClientRequest.get("/api/core/links/conflicts"),
      Schema.Struct({ conflicts: Schema.Array(ConflictingLink) }),
    ).pipe(Effect.map((r) => r.conflicts)),

    getAnimeWorksFiltered: (params) => {
      const url = params?.hasLinks
        ? "/api/core/anime-works?has_links=true"
        : "/api/core/anime-works"
      return fetchJson(
        HttpClientRequest.get(url),
        Schema.Struct({ animes: Schema.Array(AnimeWork) }),
      ).pipe(Effect.map((r) => r.animes))
    },

    search: (query) => {
      const qs = new URLSearchParams()
      qs.set("q", query)
      return fetchJson(
        HttpClientRequest.get(`/api/core/search?${qs.toString()}`),
        AggregatedSearchResponseSchema,
      )
    },

    getDetail: (detail_key, source) =>
      fetchJson(
        HttpClientRequest.post("/api/core/detail").pipe(
          HttpClientRequest.bodyUnsafeJson({ detail_key, source }),
        ),
        DetailResponseSchema,
      ),

    // AI 設定
    getAiSettings: client
      .execute(HttpClientRequest.get("/api/core/ai-settings"))
      .pipe(
        Effect.flatMap((r) => r.json),
        Effect.map((r) => r as AiSettings),
        Effect.scoped,
        Effect.orDie,
      ),

    updateAiSettings: (req) =>
      client
        .execute(
          HttpClientRequest.put("/api/core/ai-settings").pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    testAiConnection: client
      .execute(
        HttpClientRequest.post("/api/core/ai-settings/test").pipe(
          HttpClientRequest.bodyUnsafeJson({}),
        ),
      )
      .pipe(
        Effect.flatMap((r) => r.json),
        Effect.map((r) => r as { ok: boolean; error?: string }),
        Effect.scoped,
        Effect.orDie,
      ),

    getAiPromptSettings: client
      .execute(HttpClientRequest.get("/api/core/ai-prompt-settings"))
      .pipe(
        Effect.flatMap((r) => r.json),
        Effect.map((r) => r as AiPromptSettings),
        Effect.scoped,
        Effect.orDie,
      ),

    updateAiPromptSettings: (req) =>
      client
        .execute(
          HttpClientRequest.put("/api/core/ai-prompt-settings").pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    revertParserPrompt: client
      .execute(
        HttpClientRequest.post("/api/core/ai-prompt-settings/revert-parser").pipe(
          HttpClientRequest.bodyUnsafeJson({}),
        ),
      )
      .pipe(
        Effect.flatMap((r) => r.json),
        Effect.map((r) => r as { value: string }),
        Effect.scoped,
        Effect.orDie,
      ),

    revertFilterPrompt: client
      .execute(
        HttpClientRequest.post("/api/core/ai-prompt-settings/revert-filter").pipe(
          HttpClientRequest.bodyUnsafeJson({}),
        ),
      )
      .pipe(
        Effect.flatMap((r) => r.json),
        Effect.map((r) => r as { value: string }),
        Effect.scoped,
        Effect.orDie,
      ),

    // 待確認管理
    getPendingAiResults: (params) => {
      const qs = new URLSearchParams()
      if (params?.result_type) qs.set("result_type", params.result_type)
      if (params?.status) qs.set("status", params.status)
      if (params?.subscription_id != null) qs.set("subscription_id", String(params.subscription_id))
      const q = qs.toString()
      return client
        .execute(HttpClientRequest.get(`/api/core/pending-ai-results${q ? `?${q}` : ""}`))
        .pipe(
          Effect.flatMap((r) => r.json),
          Effect.map((r) => r as readonly PendingAiResult[]),
          Effect.scoped,
          Effect.orDie,
        )
    },

    getPendingAiResult: (id) =>
      client
        .execute(HttpClientRequest.get(`/api/core/pending-ai-results/${id}`))
        .pipe(
          Effect.flatMap((r) => r.json),
          Effect.map((r) => r as PendingAiResult),
          Effect.scoped,
          Effect.orDie,
        ),

    updatePendingAiResult: (id, generated_data) =>
      client
        .execute(
          HttpClientRequest.put(`/api/core/pending-ai-results/${id}`).pipe(
            HttpClientRequest.bodyUnsafeJson({ generated_data }),
          ),
        )
        .pipe(
          Effect.flatMap((r) => r.json),
          Effect.map((r) => r as PendingAiResult),
          Effect.scoped,
          Effect.orDie,
        ),

    confirmPendingAiResult: (id, req: ConfirmPendingRequest) =>
      client
        .execute(
          HttpClientRequest.post(`/api/core/pending-ai-results/${id}/confirm`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    rejectPendingAiResult: (id) =>
      client
        .execute(
          HttpClientRequest.post(`/api/core/pending-ai-results/${id}/reject`).pipe(
            HttpClientRequest.bodyUnsafeJson({}),
          ),
        )
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    regeneratePendingAiResult: (id, req: RegenerateRequest) =>
      client
        .execute(
          HttpClientRequest.post(`/api/core/pending-ai-results/${id}/regenerate`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((r) => r.json),
          Effect.map((r) => r as PendingAiResult),
          Effect.scoped,
          Effect.orDie,
        ),

  })
})

export const CoreApiLive = Layer.effect(CoreApi, makeCoreApi)
