<script setup lang="ts">
import type { TicketAccount, TicketModel } from '@/api/modules/codex-tickets'
import { ChevronRight } from '@lucide/vue'
import { useIntervalFn, useNow } from '@vueuse/core'
import { computed, ref } from 'vue'
import { BaseModal } from '@codex-proxy/ui'

const props = defineProps<{ account: TicketAccount, enabled: boolean, error: boolean }>()
const now = useNow({ scheduler: callback => useIntervalFn(callback, 1000) })
const open = ref(false)
const selectedModel = ref('')
const selected = computed(() => props.account.models.find(model => model.model === selectedModel.value))
const labels: Record<string, string> = { 'gpt-6-astra': 'astra', 'gpt-5.6-sol': 'sol' }
function remaining(model: TicketModel) {
  const seconds = Math.max(0, Math.ceil((model.expiresAt ?? 0) - now.value.getTime() / 1000))
  if (model.ready && seconds > 0)
    return `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, '0')}s`
  return !props.enabled ? '打票已关闭' : !props.account.enabled ? '调度已停止' : '暂无有效门票'
}
function ready(model: TicketModel) {
  return model.ready && (model.expiresAt ?? 0) * 1000 > now.value.getTime()
}
function rate(model: { successRate: number | null }) {
  return model.successRate == null ? '-' : `${model.successRate.toFixed(1)}%`
}
function time(at?: number) {
  return at ? new Date(at * 1000).toLocaleString() : '尚未尝试'
}
function detail(model: TicketModel) {
  selectedModel.value = model.model
  open.value = true
}
</script>

<template>
  <div class="grid min-w-0 gap-2 border-b border-cp-border-secondary pb-2 text-cp-xs">
    <span v-if="error" role="status" class="text-cp-warning-text">打票状态更新失败，当前为上次数据</span>
    <button
      v-for="model in account.models" :key="model.model" type="button"
      class="group grid min-w-0 grid-cols-[minmax(0,1fr)_1rem] gap-x-1 border-0 bg-transparent p-0 text-left text-cp-text-secondary focus-visible:outline-2 focus-visible:outline-cp-control-outline"
      :aria-label="`${labels[model.model] ?? model.model} 打票 IP 明细`" @click.stop="detail(model)"
    >
      <span class="flex min-w-0 flex-wrap gap-x-1">
        <strong>{{ labels[model.model] ?? model.model }}</strong>
        <span class="font-mono tabular-nums" :class="ready(model) ? 'text-cp-success-text' : 'text-cp-text-tertiary'">{{ remaining(model) }}</span>
      </span>
      <ChevronRight class="row-span-3 size-3.5 self-center text-cp-text-tertiary group-hover:text-cp-link" />
      <span class="min-w-0 break-words">近 1 小时 · {{ model.uniqueIps }} IP · 成功率 {{ rate(model) }}</span>
      <span class="min-w-0 break-words text-cp-text-tertiary">最近尝试 {{ time(model.lastAttempt?.at) }}</span>
      <span v-if="model.unknownIpAttempts" class="text-cp-warning-text">{{ model.unknownIpAttempts }} 次 IP 未知</span>
    </button>
  </div>
  <BaseModal v-model="open" :title="`${account.name} · ${labels[selectedModel] ?? selectedModel} 打票 IP`" size="md-wide">
    <div v-if="selected" class="grid min-w-0 gap-3 text-cp-xs">
      <p class="m-0 text-cp-text-secondary">
        近 1 小时 · {{ selected.uniqueIps }} 个已确认 IP · {{ selected.attempts }} 次尝试 · {{ selected.successes }} 次成功 · 成功率 {{ rate(selected) }}
      </p>
      <p v-if="error" role="alert" class="m-0 text-cp-warning-text">
        状态更新失败，当前为上次数据
      </p>
      <p v-if="!selected.ips.length" class="m-0 py-6 text-center text-cp-text-tertiary">
        近 1 小时暂无尝试记录
      </p>
      <div v-else class="max-h-96 overflow-auto">
        <table class="w-full border-collapse text-left">
          <thead class="sticky top-0 bg-cp-container text-cp-text-secondary">
            <tr>
              <th class="p-2">
                出口 IP
              </th><th class="p-2">
                尝试 / 成功
              </th><th class="p-2">
                成功率
              </th><th class="p-2">
                最近尝试
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="ip in selected.ips" :key="ip.ip ?? 'unknown'" class="border-t border-cp-border-secondary">
              <td class="max-w-48 break-all p-2 font-mono">
                {{ ip.ip ?? 'IP 未知' }}
              </td>
              <td class="whitespace-nowrap p-2 font-mono">
                {{ ip.attempts }} / {{ ip.successes }}
              </td>
              <td class="p-2 font-mono">
                {{ rate(ip) }}
              </td>
              <td class="p-2">
                <div>{{ time(ip.lastAttempt.at) }}</div>
                <div class="text-cp-text-tertiary">
                  HTTP {{ ip.lastAttempt.status || '-' }} · {{ ip.lastAttempt.length }} B · {{ ip.lastAttempt.success ? '成功' : '未成功' }}
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </BaseModal>
</template>
