export interface Webhook {
  webhook_id: number
  name: string
  url: string
  payload_template: string
  is_active: boolean
  /** 異常通知模板；null = 不收異常通知 */
  alert_template: string | null
  created_at: string
  updated_at: string
}

export interface CreateWebhookRequest {
  name: string
  url: string
  payload_template: string
  is_active?: boolean
  alert_template?: string
}

export interface UpdateWebhookRequest {
  name?: string
  url?: string
  payload_template?: string
  is_active?: boolean
  /** 空字串 = 不再收異常通知 */
  alert_template?: string
}
