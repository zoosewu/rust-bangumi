export interface Webhook {
  webhook_id: number
  name: string
  url: string
  payload_template: string
  is_active: boolean
  created_at: string
  updated_at: string
}

export interface CreateWebhookRequest {
  name: string
  url: string
  payload_template: string
  is_active?: boolean
}

export interface UpdateWebhookRequest {
  name?: string
  url?: string
  payload_template?: string
  is_active?: boolean
}
