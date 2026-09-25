<script setup lang="ts">
import type { AccountRow } from '../constants'
import { RefreshCw } from '@lucide/vue'
import { computed } from 'vue'

import { useUiClock } from '@/composables/useUiClock'
import { visibleSummaryQuotaWindows } from '../constants'
import AccountResetCredits from './AccountQuotaPanel/ResetCredits.vue'
import {
  accountCompactUsageSummary,
  quotaResetCountdown,
  quotaWindowCompactUsage,
  quotaWindowPresentation,
} from './AccountUsageWindow/presenter'

const props = defineProps<{ account: AccountRow, refreshing: boolean }>()
const emit = defineEmits<{
  refreshQuota: [accountId: string]
  quotaReset: [accountId: string]
}>()

const now = useUiClock()
const windows = computed(() => visibleSummaryQuotaWindows(props.account.quota.windows).slice(0, 2).map(window => ({
  ...window,
  compactLabel: window.windowSeconds === 18000
    ? '5h'
    : window.windowSeconds === 604800 ? '7d' : window.windowLabelDisplay,
  percentDisplay: typeof window.usedPercent === 'number'
    ? `${Math.round(window.usedPercent)}%`
    : window.usedPercentDisplay,
  resetCountdown: window.usedPercent === 0
    ? '现在'
    : quotaResetCountdown(window.resetAtDisplay, now.value.getTime()),
  localUsage: quotaWindowCompactUsage(window),
  presentation: quotaWindowPresentation(window, '2px'),
})))
const localUsage = computed(() => accountCompactUsageSummary(props.account))
const expiresDisplay = computed(() => {
  const value = props.account.accessTokenExpiresAtDisplay
  if (!value || value === '—' || value === '-')
    return null
  const match = value.match(/^\d{4}-(\d{2})-(\d{2}) (\d{2}:\d{2})/)
  return match ? `${match[1]}/${match[2]} ${match[3]}` : value
})

function actionClass(disabled = false) {
  return [
    'inline-flex h-cp-control-sm shrink-0 items-center justify-center rounded-cp border-0 px-1.5 text-cp-xs font-heavy outline-none transition-colors',
    disabled
      ? 'cursor-not-allowed text-cp-text-disabled'
      : 'text-cp-text-secondary hover:bg-cp-fill-quaternary hover:text-cp-text focus-visible:ring-2 focus-visible:ring-cp-control-outline',
  ]
}
</script>

<template>
  <div class="grid min-w-0 gap-1 py-1 text-cp-xs">
    <template v-if="account.authenticationKind !== 'api_key' && windows.length">
      <div
        v-for="window in windows"
        :key="window.key"
        class="grid grid-cols-[2rem_minmax(0,1fr)_2.75rem_auto] items-center gap-x-1.5 gap-y-1"
        :title="`${window.labelDisplay} · 重置：${window.resetAtDisplay}`"
      >
        <div v-if="window.localUsage.visible" class="col-span-4 flex min-w-0 flex-wrap items-center gap-1 text-[10px] leading-3">
          <span class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="窗口请求数">
            {{ window.localUsage.requestDisplay }} req
          </span>
          <span class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="窗口 Token 总数">
            {{ window.localUsage.tokensDisplay }}
          </span>
          <span v-if="window.localUsage.accountCostDisplay" class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="账户估算费用">
            A {{ window.localUsage.accountCostDisplay }}
          </span>
          <span v-if="window.localUsage.userCostDisplay" class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="用户折算费用">
            U {{ window.localUsage.userCostDisplay }}
          </span>
          <span v-if="window.localUsage.estimatedTotalCostDisplay" class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="预计总费用">
            预计总费用 {{ window.localUsage.estimatedTotalCostDisplay }}
          </span>
        </div>
        <span class="truncate text-cp-text-secondary">{{ window.compactLabel }}</span>
        <div
          class="h-1 overflow-hidden rounded-full bg-cp-border-secondary"
          role="progressbar"
          :aria-label="window.labelDisplay"
          :aria-valuenow="window.usedPercent ?? undefined"
          aria-valuemin="0"
          aria-valuemax="100"
          :aria-valuetext="window.usedPercentDisplay"
        >
          <div class="h-full rounded-full" :class="window.presentation.barClass" :style="window.presentation.barStyle" />
        </div>
        <span class="text-right font-mono tabular-nums" :class="window.presentation.percentTextClass">
          {{ window.percentDisplay }}
        </span>
        <span class="truncate text-[10px] leading-3 text-cp-text-tertiary">
          {{ window.resetCountdown || window.resetAtDisplay || '未知' }}
        </span>
      </div>
    </template>
    <span v-else class="text-cp-text-tertiary">
      {{ account.authenticationKind === 'api_key' ? '本地用量' : '额度待观测' }}
    </span>

    <div v-if="account.authenticationKind === 'api_key' && localUsage.visible" class="flex min-w-0 flex-wrap items-center gap-1 text-[10px] leading-3">
      <span class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="请求数">
        请求 {{ localUsage.requestDisplay }}
      </span>
      <span class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="Token 总数">
        Token {{ localUsage.tokensDisplay }}
      </span>
      <span v-if="localUsage.accountCostDisplay" class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="账户估算费用">
        A {{ localUsage.accountCostDisplay }}
      </span>
      <span v-if="localUsage.userCostDisplay" class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="用户折算费用">
        U {{ localUsage.userCostDisplay }}
      </span>
      <span v-if="localUsage.estimatedCostDisplay" class="rounded-cp-sm bg-cp-fill-quaternary px-1.5 py-1 font-mono tabular-nums text-cp-text-secondary" title="预计总费用">
        预计总费用 {{ localUsage.estimatedCostDisplay }}
      </span>
    </div>

    <div class="flex min-w-0 flex-wrap items-center gap-0.5 border-t border-cp-border-secondary/60 pt-0.5">
      <button
        type="button"
        :class="actionClass()"
        :aria-label="`查询 ${account.name} 的上游额度`"
        :disabled="refreshing || account.authenticationKind === 'api_key'"
        @click.stop="emit('refreshQuota', account.id)"
      >
        <RefreshCw class="mr-0.5 size-3" :class="refreshing ? 'animate-spin motion-reduce:animate-none' : ''" />
        查询
      </button>
      <AccountResetCredits
        v-if="account.provider === 'openai' && account.authenticationKind === 'oauth'"
        :account="account"
        compact
        @consumed="emit('quotaReset', $event)"
      />
      <button
        type="button"
        class="inline-flex h-cp-control-sm items-center rounded-cp border-0 bg-transparent px-1.5 text-cp-xs font-heavy text-cp-success-text outline-none transition-colors hover:bg-cp-fill-quaternary focus-visible:ring-2 focus-visible:ring-cp-control-outline disabled:cursor-not-allowed disabled:text-cp-text-disabled"
        :disabled="refreshing || account.authenticationKind === 'api_key'"
        title="查询上游账户点数"
        @click.stop="emit('refreshQuota', account.id)"
      >
        点数 {{ localUsage.creditsDisplay }}
      </button>
      <button type="button" disabled :class="actionClass(true)" title="邀请能力未由后端提供">
        可邀请 —
      </button>
      <button type="button" disabled :class="actionClass(true)" title="邀请能力未由后端提供">
        邀请用户
      </button>
      <span v-if="expiresDisplay" class="ml-auto truncate text-[10px] text-cp-text-tertiary" :title="`Access Token 到期：${expiresDisplay}`">
        到期 {{ expiresDisplay }}
      </span>
    </div>
  </div>
</template>
