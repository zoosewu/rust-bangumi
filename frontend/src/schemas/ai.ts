export type ResponseFormatMode = "strict" | "non_strict" | "inject_schema"
export type AiProviderKind = "openai_compatible"

export interface AiProvider {
  id: number
  name: string
  provider_kind: AiProviderKind
  base_url: string
  api_key: string // 列表/取得時為遮罩 ••••••••
  model_name: string
  max_tokens: number
  response_format_mode: ResponseFormatMode
  is_enabled: boolean
  priority: number
  created_at: string
  updated_at: string
}

export interface CreateAiProviderRequest {
  existing_provider_id?: number
  name: string
  provider_kind: AiProviderKind
  base_url?: string
  api_key?: string
  model_name?: string
  max_tokens?: number
  response_format_mode?: ResponseFormatMode
  is_enabled?: boolean
}

export interface UpdateAiProviderRequest {
  name?: string
  provider_kind?: AiProviderKind
  base_url?: string
  api_key?: string // 空字串 = 不更新（保留舊值）
  model_name?: string
  max_tokens?: number
  response_format_mode?: ResponseFormatMode
  is_enabled?: boolean
}

/** 取得模型列表：編輯既有 provider 時 api_key 留空就用它已存的 key */
export interface ListAiModelsRequest {
  existing_provider_id?: number
  base_url: string
  api_key: string
}

export type PromptTestKind = "parser" | "filter"

/** 用真實 prompt 測試：provider 設定（與測試連線相同）＋類型＋訂閱 */
export interface PromptTestRequest {
  existing_provider_id?: number
  provider_kind: AiProviderKind
  base_url: string
  api_key: string
  model_name: string
  max_tokens: number
  response_format_mode: ResponseFormatMode
  kind: PromptTestKind
  subscription_id: number
}

export interface TitleEvaluation {
  title: string
  /** parser: match / no_match；filter: keep / drop */
  outcome: "match" | "no_match" | "keep" | "drop"
  detail: string | null
}

export interface PromptTestResult {
  ok: boolean
  elapsed_ms: number
  system_prompt: string
  user_prompt: string
  titles: string[]
  raw_response: string | null
  parsed: unknown
  evaluations: TitleEvaluation[]
  error: string | null
}

export interface TestAiProviderResult {
  ok: boolean
  error?: string
}

export type TestAiProviderConfigRequest = CreateAiProviderRequest

export interface AiPromptSettings {
  id: number
  fixed_parser_prompt: string | null
  fixed_filter_prompt: string | null
  custom_parser_prompt: string | null
  custom_filter_prompt: string | null
  max_concurrent_requests: number
  created_at: string
  updated_at: string
}

export interface PendingAiResult {
  id: number
  result_type: "parser" | "filter"
  source_title: string
  generated_data: Record<string, unknown> | null
  status: "generating" | "pending" | "confirmed" | "failed"
  error_message: string | null
  raw_item_id: number | null
  subscription_id: number | null
  used_fixed_prompt: string
  used_custom_prompt: string | null
  expires_at: string | null
  created_at: string
  updated_at: string
  confirm_level: "global" | "subscription" | "anime_work" | null
  confirm_target_id: number | null
}

export interface ConfirmPendingRequest {
  level: "global" | "subscription" | "anime_work"
  target_id?: number
}

export interface RegenerateRequest {
  custom_prompt?: string
  fixed_prompt?: string
}
