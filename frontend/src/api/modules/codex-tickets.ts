import type { RequestOptions } from '../request'
import request from '../request'

export interface TicketAttempt {
  at: number
  ip: string | null
  status: number
  length: number
  success: boolean
  result: string
}

export interface TicketIpSummary {
  ip: string | null
  attempts: number
  successes: number
  successRate: number
  lastAttempt: TicketAttempt
}

export interface TicketModel {
  model: string
  ready: boolean
  blocked: boolean
  remainingSeconds: number
  expiresAt: number | null
  windowSeconds: number
  maxAttempts: number
  attempts: number
  successes: number
  successRate: number | null
  uniqueIps: number
  unknownIpAttempts: number
  lastAttempt: TicketAttempt | null
  ips: TicketIpSummary[]
}

export interface TicketAccount {
  accountId: string
  name: string
  enabled: boolean
  models: TicketModel[]
}

export interface TicketSettings {
  enabled: boolean
  revision: number
  accountIds: string[]
  proxyConfigured: boolean
  proxyEndpoint: string | null
  accounts: TicketAccount[]
}

export function getCodexTickets(options?: RequestOptions) {
  return request<TicketSettings>({
    url: '/api/admin/accounts/codex-tickets',
    method: 'GET',
    ...options,
  })
}
