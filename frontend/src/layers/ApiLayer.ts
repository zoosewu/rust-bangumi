import { Effect, Layer, Schema } from "effect"
import * as HttpClient from "@effect/platform/HttpClient"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"
import { CoreApi } from "@/services/CoreApi"
import { Anime, AnimeSeries, Season, SubtitleGroup, AnimeLink, AnimeSeriesRich, AnimeLinkRich } from "@/schemas/anime"
import { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import { TitleParser, ParserPreviewResponse } from "@/schemas/parser"
import { Subscription } from "@/schemas/subscription"
import { RawAnimeItem, DownloadRow } from "@/schemas/download"
import { DashboardStats } from "@/schemas/dashboard"

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
    getAnimes: fetchJson(
      HttpClientRequest.get("/api/core/anime"),
      Schema.Struct({ animes: Schema.Array(Anime) }),
    ).pipe(Effect.map((r) => r.animes)),

    createAnime: (title) => postJson("/api/core/anime", { title }, Anime),

    deleteAnime: (id) =>
      client
        .execute(HttpClientRequest.del(`/api/core/anime/${id}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getSubscriptions: fetchJson(
      HttpClientRequest.get("/api/core/subscriptions"),
      Schema.Struct({ subscriptions: Schema.Array(Subscription) }),
    ).pipe(Effect.map((r) => r.subscriptions)),

    getFilterRules: (targetType, targetId) =>
      fetchJson(
        HttpClientRequest.get(
          `/api/core/filters?target_type=${targetType}${targetId != null ? `&target_id=${targetId}` : ""}`,
        ),
        Schema.Struct({ rules: Schema.Array(FilterRule) }),
      ).pipe(Effect.map((r) => r.rules)),

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
      postJson("/api/core/parsers", req, TitleParser),

    deleteParser: (id) =>
      client
        .execute(HttpClientRequest.del(`/api/core/parsers/${id}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    previewParser: (req) =>
      postJson("/api/core/parsers/preview", req, ParserPreviewResponse),

    getRawItems: (params) => {
      const qs = new URLSearchParams()
      if (params.status) qs.set("status", params.status)
      if (params.subscription_id != null)
        qs.set("subscription_id", String(params.subscription_id))
      if (params.limit != null) qs.set("limit", String(params.limit))
      if (params.offset != null) qs.set("offset", String(params.offset))
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

    getConflicts: fetchJson(
      HttpClientRequest.get("/api/core/conflicts"),
      Schema.Struct({ conflicts: Schema.Array(Schema.Any) }),
    ).pipe(Effect.map((r) => r.conflicts as readonly Record<string, unknown>[])),

    resolveConflict: (conflictId, fetcherId) =>
      postJson(
        `/api/core/conflicts/${conflictId}/resolve`,
        { fetcher_id: fetcherId },
        Schema.Any,
      ),

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

    getAnimeSeries: (animeId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/anime/${animeId}/series`),
        Schema.Struct({ series: Schema.Array(AnimeSeries) }),
      ).pipe(Effect.map((r) => r.series)),

    getOneAnimeSeries: (seriesId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/anime/series/${seriesId}`),
        AnimeSeries,
      ),

    createAnimeSeries: (req) =>
      postJson("/api/core/anime/series", req, AnimeSeries),

    getSeasons: fetchJson(
      HttpClientRequest.get("/api/core/seasons"),
      Schema.Struct({ seasons: Schema.Array(Season) }),
    ).pipe(Effect.map((r) => r.seasons)),

    createSeason: (req) =>
      postJson("/api/core/seasons", req, Season),

    getAnimeLinks: (seriesId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/links/${seriesId}`),
        Schema.Struct({ links: Schema.Array(AnimeLink) }),
      ).pipe(Effect.map((r) => r.links)),

    getAllAnimeSeries: fetchJson(
      HttpClientRequest.get("/api/core/series"),
      Schema.Struct({ series: Schema.Array(AnimeSeriesRich) }),
    ).pipe(Effect.map((r) => r.series)),

    getDashboardStats: fetchJson(
      HttpClientRequest.get("/api/core/dashboard/stats"),
      DashboardStats,
    ),

    getAnimeLinksRich: (seriesId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/links/${seriesId}`),
        Schema.Struct({ links: Schema.Array(AnimeLinkRich) }),
      ).pipe(Effect.map((r) => r.links)),

    updateAnimeSeries: (seriesId, req) =>
      client
        .execute(
          HttpClientRequest.put(`/api/core/anime/series/${seriesId}`).pipe(
            HttpClientRequest.bodyUnsafeJson(req),
          ),
        )
        .pipe(
          Effect.flatMap((response) => response.json),
          Effect.flatMap(Schema.decodeUnknown(AnimeSeries)),
          Effect.scoped,
          Effect.orDie,
        ),

    getRawItem: (itemId) =>
      fetchJson(
        HttpClientRequest.get(`/api/core/raw-items/${itemId}`),
        RawAnimeItem,
      ),

    createSubscription: (req) =>
      postJson("/api/core/subscriptions", req, Subscription),

    deleteSubscription: (id) =>
      client
        .execute(HttpClientRequest.del(`/api/core/subscriptions/${id}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getRawItemsCount: (subscriptionId, status) =>
      fetchJson(
        HttpClientRequest.get(
          `/api/core/raw-items/count?subscription_id=${subscriptionId}&status=${status}`,
        ),
        Schema.Struct({ count: Schema.Number }),
      ).pipe(Effect.map((r) => r.count)),
  })
})

export const CoreApiLive = Layer.effect(CoreApi, makeCoreApi)
