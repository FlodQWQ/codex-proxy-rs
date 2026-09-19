<script setup lang="ts">
import type { AccountRow } from '../constants'
import { RefreshCw } from '@lucide/vue'
import { computed } from 'vue'
import BaseIconButton from '@/components/base/BaseIconButton.vue'
import { visibleSummaryQuotaWindows } from '../constants'
import { quotaWindowPresentation } from './AccountUsageWindow/presenter'

const props = defineProps<{ account: AccountRow, refreshing: boolean }>()
const emit = defineEmits<{ refreshQuota: [accountId: string] }>()
const windows = computed(() => visibleSummaryQuotaWindows(props.account.quota.windows).slice(0, 2).map(window => ({
  ...window,
  compactLabel: window.windowSeconds === 18000 ? '5h' : window.windowSeconds === 604800 ? '7d' : window.windowLabelDisplay,
  presentation: quotaWindowPresentation(window, '2px'),
})))
</script>

<template>
  <div class="grid min-w-0 gap-2 py-1 text-cp-xs">
    <template v-if="account.authenticationKind !== 'api_key' && windows.length">
      <div v-for="window in windows" :key="window.key" class="grid grid-cols-[2rem_minmax(0,1fr)_2.5rem] items-center gap-2" :title="`${window.labelDisplay} · 重置：${window.resetAtDisplay}`">
        <span class="truncate text-cp-text-secondary">{{ window.compactLabel }}</span>
        <div
          class="h-1 overflow-hidden rounded-full bg-cp-border-secondary" role="progressbar"
          :aria-label="window.labelDisplay" :aria-valuenow="window.usedPercent ?? undefined"
          aria-valuemin="0" aria-valuemax="100" :aria-valuetext="window.usedPercentDisplay"
        >
          <div class="h-full rounded-full" :class="window.presentation.barClass" :style="window.presentation.barStyle" />
        </div>
        <span class="text-right font-mono tabular-nums" :class="window.presentation.percentTextClass">{{ window.usedPercentDisplay }}</span>
        <span class="col-span-3 min-w-0 break-words text-cp-text-tertiary">
          {{ window.compactLabel }} 重置：{{ window.resetAtDisplay || '未知' }}
        </span>
      </div>
    </template>
    <span v-else class="text-cp-text-tertiary">{{ account.authenticationKind === 'api_key' ? '本地用量' : '额度待观测' }}</span>
    <div v-if="account.authenticationKind === 'oauth'" class="flex min-w-0 items-center justify-between gap-1 text-cp-text-tertiary">
      <span class="min-w-0 truncate" :title="`上游配额最近更新：${account.quota.refreshedAtDisplay}`">
        更新：{{ account.quota.refreshedAtDisplay || '尚未查询' }}
      </span>
      <BaseIconButton
        label="向上游查询配额" size="sm" variant="ghost"
        :loading="refreshing" :disabled="refreshing"
        @click.stop="emit('refreshQuota', account.id)"
      >
        <RefreshCw class="size-3.5" />
      </BaseIconButton>
    </div>
    <span class="truncate text-cp-text-secondary" :title="account.usage.windowLabelDisplay">
      <span class="font-mono tabular-nums">{{ account.usage.totalTokensDisplay }}</span> Tokens
      <span class="text-cp-text-tertiary"> · {{ account.usage.requestCountDisplay }} 次</span>
    </span>
  </div>
</template>
