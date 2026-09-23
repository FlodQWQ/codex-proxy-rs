import type { RequestOptions } from '../request'
import request from '../request'

export interface ModelFingerprintModel {
  id: string
  label: string
}

export interface ModelFingerprintTestResult {
  accountId: string
  sentModel: string
  responseModel: string | null
  confidence: number | null
  status: 'degraded' | 'passed' | 'inconclusive'
  attempted: number
  usedOutputs: number
  expiresAt: string | null
}

export function getFingerprintModels(options?: RequestOptions) {
  return request<{ models: ModelFingerprintModel[] }>({
    url: '/api/admin/accounts/fingerprint/models',
    method: 'GET',
    timeout: 600_000,
    ...options,
  })
}

export function testAccountFingerprint(
  data: { accountId: string, modelId: string },
  options?: RequestOptions,
) {
  return request<ModelFingerprintTestResult>({
    url: '/api/admin/accounts/fingerprint-test',
    method: 'POST',
    data,
    ...options,
  })
}
