export interface AiSettings {
  id: number
  base_url: string
  api_key: string // already masked
  model_name: string
  created_at: string
  updated_at: string
}

export interface AiPromptSettings {
  id: number
  fixed_parser_prompt: string | null
  fixed_filter_prompt: string | null
  custom_parser_prompt: string | null
  custom_filter_prompt: string | null
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
  used_fixed_prompt: string
  used_custom_prompt: string | null
  expires_at: string | null
  created_at: string
  updated_at: string
}

export interface ConfirmPendingRequest {
  level: "global" | "subscription" | "anime_work"
  target_id?: number
}

export interface RegenerateRequest {
  custom_prompt?: string
}
