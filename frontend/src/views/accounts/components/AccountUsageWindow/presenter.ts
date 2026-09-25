import type { AccountQuotaWindow, AccountRow } from '../../constants'
import { clamp } from 'es-toolkit'
import { formatInteger } from '@/utils/number'
import { isRecord } from '@/utils/object'

export type AccountUsageWindowVariant = 'compact' | 'detail' | 'metric'

type AccountUsageWindowMode = 'quota' | 'local' | 'unknown'

interface AccountRequestBucket {
  bucketStart: string
  requestCount: number
}

export interface AccountRequestBar {
  key: string
  requestCount: number
  height: string
  title: string
}

interface AccountLocalUsage {
  requestCount?: number
  requestCountDisplay?: string
  totalTokens?: number
  totalTokensDisplay?: string
  costEstimateStatus?: string
  costs?: Array<{ currency: string, estimatedAmountDisplay: string }>
  userCostMultiplier?: string
  userCosts?: Array<{ currency: string, estimatedAmountDisplay: string }>
  requestBuckets?: AccountRequestBucket[]
}

interface AccountUsageWindowVariantDefinition {
  rootClass: string
  headerClass: string
  labelClass: string
  valueClass: string
  trackOffsetClass: string
  trackShapeClass: string
  minimumBarWidth: string
  compactValues: boolean
  labelTooltip: boolean
  showResetTime: boolean
}

type QuotaWindowTone = 'danger' | 'success' | 'unknown' | 'warning'

export interface AccountUsageWindowPresentationInput {
  window: AccountQuotaWindow | undefined
  variant: AccountUsageWindowVariant
  showLocalValue: boolean
  now: number
}

const variantDefinitions: Record<
  AccountUsageWindowVariant,
  AccountUsageWindowVariantDefinition
> = {
  metric: {
    rootClass: 'grid min-w-0 grid-rows-[14px_14px] gap-1',
    headerClass: 'flex items-center justify-between gap-2 text-cp-xs leading-[14px] font-bold',
    labelClass: 'truncate text-cp-text-quaternary',
    valueClass: 'text-cp-xs leading-none font-bold',
    trackOffsetClass: 'self-center',
    trackShapeClass: 'h-1.5 w-full overflow-hidden rounded-full',
    minimumBarWidth: '6px',
    compactValues: false,
    labelTooltip: true,
    showResetTime: false,
  },
  compact: {
    rootClass: 'flex min-w-0 flex-col',
    headerClass: 'mb-1 flex items-baseline justify-between gap-2 text-cp-xs leading-[14px] font-bold',
    labelClass: 'truncate text-cp-text-quaternary',
    valueClass: 'text-[10px] leading-[14px] font-heavy',
    trackOffsetClass: 'mt-auto',
    trackShapeClass: 'h-[3px] w-full overflow-hidden rounded-full',
    minimumBarWidth: '3px',
    compactValues: true,
    labelTooltip: true,
    showResetTime: false,
  },
  detail: {
    rootClass: 'rounded-lg bg-cp-fill-quaternary p-2',
    headerClass: 'flex items-center justify-between gap-3 text-cp-sm font-bold',
    labelClass: 'text-cp-text-secondary',
    valueClass: 'text-cp-sm font-heavy',
    trackOffsetClass: 'mt-2',
    trackShapeClass: 'h-2 overflow-hidden rounded-full',
    minimumBarWidth: '8px',
    compactValues: false,
    labelTooltip: false,
    showResetTime: true,
  },
}

const hourMilliseconds = 60 * 60 * 1_000
const hourFormatter = new Intl.DateTimeFormat('zh-CN', {
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
})
const quotaToneClasses: Record<
  QuotaWindowTone,
  { bar: string, text: string }
> = {
  unknown: {
    bar: 'bg-cp-border',
    text: 'text-cp-text-quaternary',
  },
  danger: {
    bar: 'bg-cp-error',
    text: 'text-cp-error-text',
  },
  warning: {
    bar: 'bg-cp-warning',
    text: 'text-cp-warning-text',
  },
  success: {
    bar: 'bg-cp-success',
    text: 'text-cp-success-text',
  },
}

export function resolveAccountUsageWindowPresentation(
  input: AccountUsageWindowPresentationInput,
) {
  const definition = variantDefinitions[input.variant]
  const localUsage = accountLocalUsage(input.window?.localUsage)
  const mode = usageWindowMode(input.window, localUsage)
  const quotaLocalUsageDisplay = localTokenDisplay(localUsage)
  const quotaLocalUsageVisible = input.showLocalValue && Boolean(quotaLocalUsageDisplay)
  const localRequestDisplay = requestCountDisplay(localUsage)
  const localRequestValueVisible = input.showLocalValue
    && typeof localUsage?.requestCount === 'number'
    && localUsage.requestCount > 0
  const localRequestLabel = '日请求'
  const quota = input.window
    ? quotaWindowPresentation(input.window, definition.minimumBarWidth)
    : null

  return {
    mode,
    compactValues: definition.compactValues,
    labelTooltip: definition.labelTooltip ? input.window?.labelDisplay : undefined,
    classes: {
      root: definition.rootClass,
      header: definition.headerClass,
      label: definition.labelClass,
      value: definition.valueClass,
      trackOffset: definition.trackOffsetClass,
      trackShape: definition.trackShapeClass,
      track: `${definition.trackShapeClass} bg-cp-border-secondary`,
    },
    quota: {
      valueVisible: typeof input.window?.usedPercent === 'number'
        && (input.variant !== 'compact' || input.window.usedPercent > 0),
      localUsageDisplay: quotaLocalUsageDisplay,
      localUsageVisible: quotaLocalUsageVisible,
      barStyle: quota?.barStyle,
      barClass: quota?.barClass,
      percentTextClass: quota?.percentTextClass,
      resetVisible: Boolean(input.window)
        && definition.showResetTime
        && input.window?.resetAtDisplay !== '—',
    },
    local: {
      label: localRequestLabel,
      requestDisplay: localRequestDisplay,
      requestValueVisible: localRequestValueVisible,
      timelineTitle: `${localRequestLabel} ${localRequestDisplay} 次`,
      requestBars: requestTimeline(localUsage?.requestBuckets ?? [], input.now),
      durationDisplay: rollingWindowDurationDisplay(input.window?.windowSeconds),
    },
  }
}

function accountLocalUsage(value: unknown): AccountLocalUsage | null {
  if (!isRecord(value))
    return null

  const localUsage: AccountLocalUsage = {
    requestBuckets: requestBuckets(value.requestBuckets),
  }
  if (finiteNumber(value.requestCount))
    localUsage.requestCount = value.requestCount
  if (typeof value.requestCountDisplay === 'string')
    localUsage.requestCountDisplay = value.requestCountDisplay
  if (finiteNumber(value.totalTokens))
    localUsage.totalTokens = value.totalTokens
  if (typeof value.totalTokensDisplay === 'string')
    localUsage.totalTokensDisplay = value.totalTokensDisplay
  if (typeof value.costEstimateStatus === 'string')
    localUsage.costEstimateStatus = value.costEstimateStatus
  if (Array.isArray(value.costs))
    localUsage.costs = currencyCosts(value.costs)
  if (typeof value.userCostMultiplier === 'string')
    localUsage.userCostMultiplier = value.userCostMultiplier
  if (Array.isArray(value.userCosts))
    localUsage.userCosts = currencyCosts(value.userCosts)
  return localUsage
}

function currencyCosts(value: unknown[]) {
  return value.flatMap((cost) => {
    if (!isRecord(cost) || typeof cost.currency !== 'string' || typeof cost.estimatedAmountDisplay !== 'string')
      return []
    return [{ currency: cost.currency, estimatedAmountDisplay: cost.estimatedAmountDisplay }]
  })
}

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function requestBuckets(value: unknown) {
  if (!Array.isArray(value))
    return []

  return value.flatMap((item) => {
    if (!isRecord(item))
      return []
    if (
      typeof item.bucketStart !== 'string'
      || !finiteNumber(item.requestCount)
      || !Number.isFinite(new Date(item.bucketStart).getTime())
    ) {
      return []
    }
    return [{
      bucketStart: item.bucketStart,
      requestCount: item.requestCount,
    }]
  })
}

function usageWindowMode(
  window: AccountQuotaWindow | undefined,
  localUsage: AccountLocalUsage | null,
): AccountUsageWindowMode {
  if (typeof window?.usedPercent === 'number')
    return 'quota'
  if (localUsage)
    return 'local'
  return window ? 'quota' : 'unknown'
}

function localTokenDisplay(localUsage: AccountLocalUsage | null) {
  const total = localUsage?.totalTokens
  if (typeof total !== 'number' || total <= 0)
    return ''

  const display = localUsage?.totalTokensDisplay
  if (typeof display === 'string' && display.trim())
    return display.trim()
  return formatInteger(total)
}

export function quotaWindowLocalUsageDisplay(window: AccountQuotaWindow) {
  const localUsage = accountLocalUsage(window.localUsage)
  return localTokenDisplay(localUsage) || null
}

export function quotaWindowCostDisplays(window: AccountQuotaWindow) {
  const localUsage = accountLocalUsage(window.localUsage)
  if (!localUsage?.costEstimateStatus)
    return null
  const accountCost = localUsage.costs?.find(item => item.currency.toUpperCase() === 'USD')
  const userCost = localUsage.userCosts?.find(item => item.currency.toUpperCase() === 'USD')
  if (!accountCost && !userCost && !localUsage.requestCount)
    return null
  const suffix = localUsage.costEstimateStatus === 'partial' ? '（部分费用）' : ''
  const known = localUsage.costEstimateStatus !== 'unknown'
  return {
    account: known && accountCost ? `${accountCost.estimatedAmountDisplay}${suffix}` : '—（无法计价）',
    user: localUsage.userCostMultiplier
      ? (known && userCost ? `${userCost.estimatedAmountDisplay}${suffix}` : '—（无法计价）')
      : null,
    multiplier: localUsage.userCostMultiplier,
  }
}

export interface AccountCompactUsageSummary {
  visible: boolean
  requestDisplay: string
  tokensDisplay: string
  accountCostDisplay: string | null
  userCostDisplay: string | null
  estimatedCostDisplay: string | null
  creditsDisplay: string
}

export interface QuotaWindowCompactUsage {
  visible: boolean
  requestDisplay: string
  tokensDisplay: string
  accountCostDisplay: string | null
  userCostDisplay: string | null
  estimatedCostDisplay: string | null
}

export function quotaWindowCompactUsage(window: AccountQuotaWindow): QuotaWindowCompactUsage {
  const usage = accountLocalUsage(window.localUsage)
  const costs = quotaWindowCostDisplays(window)
  const requestCount = usage?.requestCount ?? 0
  const tokens = usage?.totalTokens ?? 0
  return {
    visible: requestCount > 0 || tokens > 0 || Boolean(costs),
    requestDisplay: requestCountDisplay(usage),
    tokensDisplay: localTokenDisplay(usage) || '0',
    accountCostDisplay: costs?.account ?? null,
    userCostDisplay: costs?.user ?? null,
    // “预计”沿用账号侧的本地估算；user 是同一事实的折算展示，不能相加或重复当作额外费用。
    estimatedCostDisplay: costs?.account || null,
  }
}

/**
 * 用账号列表需要的最小字段构造紧凑用量摘要。
 * 成本优先使用账号汇总；用户折算成本来自额度窗口的本地估算，避免重复实现解析规则。
 */
export function accountCompactUsageSummary(account: Pick<AccountRow, 'usage' | 'quota'>): AccountCompactUsageSummary {
  const usage = accountLocalUsage(account.usage)
  const windowCost = account.quota.windows
    .map(quotaWindowCostDisplays)
    .find(cost => cost !== null)
  const accountCost = usage?.costs?.find(item => item.currency.toUpperCase() === 'USD')
  const accountCostDisplay = accountCost?.estimatedAmountDisplay
    || windowCost?.account
    || null
  const userCostDisplay = windowCost?.user ?? null
  const estimatedCostDisplay = accountCostDisplay
  const requestCount = usage?.requestCount ?? 0
  const totalTokens = usage?.totalTokens ?? 0

  return {
    visible: requestCount > 0 || totalTokens > 0 || Boolean(accountCostDisplay || userCostDisplay),
    requestDisplay: requestCountDisplay(usage),
    tokensDisplay: localTokenDisplay(usage) || '0',
    accountCostDisplay,
    userCostDisplay,
    estimatedCostDisplay,
    creditsDisplay: account.quota.credits
      ? account.quota.credits.unlimited
        ? '无限'
        : account.quota.credits.hasCredits === false ? '0' : account.quota.credits.balanceDisplay
      : '—',
  }
}

export function quotaResetCountdown(resetAtDisplay: string, now: number) {
  if (!resetAtDisplay || resetAtDisplay === '—')
    return null
  const resetAt = new Date(resetAtDisplay).getTime()
  if (!Number.isFinite(resetAt))
    return null
  const remainingSeconds = Math.max(0, Math.ceil((resetAt - now) / 1_000))
  if (remainingSeconds <= 0)
    return '已到期'
  const days = Math.floor(remainingSeconds / 86_400)
  const hours = Math.floor((remainingSeconds % 86_400) / 3_600)
  const minutes = Math.floor((remainingSeconds % 3_600) / 60)
  if (days > 0)
    return `${days}天${hours}小时后`
  if (hours > 0)
    return `${hours}小时${minutes}分后`
  return `${Math.max(1, minutes)}分钟后`
}

function requestCountDisplay(localUsage: AccountLocalUsage | null) {
  const display = localUsage?.requestCountDisplay
  if (typeof display === 'string' && display.trim())
    return display.trim()
  const count = localUsage?.requestCount
  return typeof count === 'number' ? formatInteger(count) : '0'
}

function rollingWindowDurationDisplay(windowSeconds: number | null | undefined) {
  if (typeof windowSeconds !== 'number' || !Number.isFinite(windowSeconds) || windowSeconds <= 0)
    return '—'

  const hourSeconds = 60 * 60
  const daySeconds = 24 * hourSeconds
  if (windowSeconds <= daySeconds * 2 && windowSeconds % hourSeconds === 0)
    return `${formatInteger(windowSeconds / hourSeconds)} 小时`
  if (windowSeconds % daySeconds === 0)
    return `${formatInteger(windowSeconds / daySeconds)} 天`
  if (windowSeconds % hourSeconds === 0)
    return `${formatInteger(windowSeconds / hourSeconds)} 小时`
  return `${formatInteger(windowSeconds)} 秒`
}

export function quotaWindowPresentation(window: AccountQuotaWindow, minimumWidth: string) {
  const percent = clamp(window.usedPercent ?? 0, 0, 100)
  const tone = quotaWindowTone(window.usedPercent)
  return {
    barStyle: {
      width: `${percent}%`,
      minWidth: percent > 0 ? minimumWidth : '0',
    },
    barClass: quotaToneClasses[tone].bar,
    percentTextClass: quotaToneClasses[tone].text,
  }
}

function quotaWindowTone(usedPercent: number | null): QuotaWindowTone {
  if (usedPercent === null)
    return 'unknown'
  if (usedPercent >= 95)
    return 'danger'
  if (usedPercent >= 80)
    return 'warning'
  return 'success'
}

function requestTimeline(
  buckets: NonNullable<AccountLocalUsage['requestBuckets']>,
  now: number,
): AccountRequestBar[] {
  const currentHour = Math.floor(now / hourMilliseconds) * hourMilliseconds
  const bucketCounts = new Map(
    buckets.map((bucket) => {
      const bucketHour = Math.floor(new Date(bucket.bucketStart).getTime() / hourMilliseconds)
        * hourMilliseconds
      return [bucketHour, Math.max(0, bucket.requestCount)] as const
    }),
  )
  const slots = Array.from({ length: 24 }, (_, index) => {
    const startTime = currentHour - (23 - index) * hourMilliseconds
    return {
      startTime,
      requestCount: bucketCounts.get(startTime) ?? 0,
    }
  })
  const maximum = Math.max(1, ...slots.map(slot => slot.requestCount))

  return slots.map((slot) => {
    const start = new Date(slot.startTime)
    const end = new Date(slot.startTime + hourMilliseconds)
    return {
      key: start.toISOString(),
      requestCount: slot.requestCount,
      height: slot.requestCount === 0
        ? '0'
        : `${Math.max(25, Math.round(slot.requestCount / maximum * 100))}%`,
      title: `${hourFormatter.format(start)}–${hourFormatter.format(end)} · ${slot.requestCount} 次请求`,
    }
  })
}
