<script setup lang="ts">
import type { getAccounts } from '@/api'
import { computed } from 'vue'
import { formatInteger } from '@/utils/number'

const props = defineProps<{
  summary: Awaited<ReturnType<typeof getAccounts>>['summary']
}>()
const items = computed(() => [
  { label: '全部账号', value: props.summary.total, tone: 'text-cp-text' },
  { label: '正常', value: props.summary.normal, tone: 'text-cp-success-text' },
  { label: '配额耗尽', value: props.summary.quotaExhausted, tone: 'text-cp-warning-text' },
  { label: '限流', value: props.summary.rateLimited, tone: 'text-cp-warning-text' },
  { label: '已停用', value: props.summary.disabled, tone: 'text-cp-text-secondary' },
  { label: '错误', value: props.summary.error, tone: 'text-cp-error-text' },
])
</script>

<template>
  <dl class="m-0 flex shrink-0 flex-wrap items-center gap-x-5 gap-y-2 border-y border-cp-border-secondary py-3 text-cp-sm" aria-label="账号状态汇总">
    <div v-for="item in items" :key="item.label" class="flex items-baseline gap-2">
      <dt class="text-cp-text-secondary">
        {{ item.label }}
      </dt>
      <dd class="m-0 font-mono font-bold tabular-nums" :class="item.tone">
        {{ formatInteger(item.value ?? 0) }}
      </dd>
    </div>
  </dl>
</template>
