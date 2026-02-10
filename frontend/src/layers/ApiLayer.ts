import { Effect, Layer, Schema } from "effect"
import * as HttpClient from "@effect/platform/HttpClient"
import * as HttpClientRequest from "@effect/platform/HttpClientRequest"
import { CoreApi } from "@/services/CoreApi"
import { Anime } from "@/schemas/anime"
import { FilterRule, FilterPreviewResponse } from "@/schemas/filter"
import { TitleParser, ParserPreviewResponse } from "@/schemas/parser"
import { Subscription } from "@/schemas/subscription"
import { RawAnimeItem } from "@/schemas/download"

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
      Schema.Array(Anime),
    ),

    createAnime: (title) => postJson("/api/core/anime", { title }, Anime),

    deleteAnime: (id) =>
      client
        .execute(HttpClientRequest.del(`/api/core/anime/${id}`))
        .pipe(Effect.asVoid, Effect.scoped, Effect.orDie),

    getSubscriptions: fetchJson(
      HttpClientRequest.get("/api/core/subscriptions"),
      Schema.Array(Subscription),
    ),

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

    getParsers: fetchJson(
      HttpClientRequest.get("/api/core/parsers"),
      Schema.Array(TitleParser),
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
      return fetchJson(
        HttpClientRequest.get(`/api/core/raw-items?${qs.toString()}`),
        Schema.Array(RawAnimeItem),
      )
    },

    getHealth: fetchJson(
      HttpClientRequest.get("/api/core/health"),
      Schema.Struct({ status: Schema.String, service: Schema.String }),
    ),
  })
})

export const CoreApiLive = Layer.effect(CoreApi, makeCoreApi)
