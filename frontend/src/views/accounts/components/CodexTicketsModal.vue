<script setup lang="ts">
import type { TicketSettings } from '@/api/modules/codex-tickets'
import { RefreshCw } from '@lucide/vue'
import { onScopeDispose, ref, watch } from 'vue'
import { getAccounts } from '@/api'
import request from '@/api/request'
import { BaseButton, BaseCheckbox, BaseFormItem, BaseIconButton, BaseInput, BaseModal, BaseSwitch, toast } from '@codex-proxy/ui'

const emit = defineEmits<{ saved: [] }>()
const open = defineModel<boolean>({ required: true })
const loading = ref(false)
const saving = ref(false)
const error = ref(false)
const state = ref<TicketSettings | null>(null)
const enabled = ref(false)
const proxyUrl = ref('')
const selected = ref<string[]>([])
const editRevision = ref(0)
const initialized = ref(false)
const accounts = ref<Awaited<ReturnType<typeof getAccounts>>['items']>([])
let timer: ReturnType<typeof setInterval> | undefined
let controller: AbortController | undefined

async function load(initial = false) {
  if (loading.value || saving.value)
    return
  loading.value = true
  error.value = false
  const active = new AbortController()
  controller = active
  try {
    const result = await request<TicketSettings>({ url: '/api/admin/accounts/codex-tickets', method: 'GET', signal: active.signal })
    if (active.signal.aborted)
      return
    state.value = result
    if (initial) {
      editRevision.value = result.revision
      enabled.value = result.enabled
      selected.value = [...result.accountIds]
      proxyUrl.value = ''
      const first = await getAccounts({ page: 1, pageSize: 200, provider: 'openai' }, { signal: active.signal })
      const rows = [...first.items]
      for (let page = 2; page <= first.page.totalPages; page++) {
        const next = await getAccounts({ page, pageSize: 200, provider: 'openai' }, { signal: active.signal })
        rows.push(...next.items)
      }
      if (!active.signal.aborted) {
        accounts.value = rows.filter(account => account.authenticationKind === 'oauth')
        initialized.value = true
      }
    }
  }
  catch {
    if (!active.signal.aborted)
      error.value = true
  }
  finally {
    if (controller === active)
      loading.value = false
  }
}

function select(id: string, value: boolean) {
  selected.value = value ? [...new Set([...selected.value, id])] : selected.value.filter(item => item !== id)
}

async function save() {
  if (!state.value || saving.value)
    return
  saving.value = true
  try {
    state.value = await request<TicketSettings>({
      url: '/api/admin/accounts/codex-tickets',
      method: 'POST',
      data: { enabled: enabled.value, accountIds: selected.value, proxyUrl: proxyUrl.value, revision: editRevision.value },
    })
    proxyUrl.value = ''
    editRevision.value = state.value.revision
    toast.success('打票设置已保存')
    emit('saved')
  }
  catch { /* 请求层显示具体错误，保留表单便于修正。 */ }
  finally { saving.value = false }
}

watch(open, (value) => {
  controller?.abort()
  controller = undefined
  loading.value = false
  clearInterval(timer)
  if (value) {
    initialized.value = false
    state.value = null
    void load(true)
    timer = setInterval(() => {
      if (initialized.value)
        void load()
    }, 10000)
  }
})
onScopeDispose(() => {
  controller?.abort()
  clearInterval(timer)
})
function date(at?: number) {
  return at ? new Date(at * 1000).toLocaleString() : '尚未尝试'
}
</script>

<template>
  <BaseModal v-model="open" title="Codex 292 打票" size="md-wide" :dismissible="!saving">
    <div class="grid gap-4">
      <div v-if="error" role="alert" class="text-cp-error-text">
        打票状态读取失败
      </div>
      <div class="flex items-center justify-between">
        <span class="font-bold">总开关</span>
        <BaseSwitch v-model="enabled" label="292 打票总开关" :disabled="loading || saving || !state" />
      </div>
      <BaseFormItem label="打票专用代理">
        <BaseInput id="codex-harvest-proxy" v-model="proxyUrl" type="text" autocomplete="off" :spellcheck="false" placeholder="完整 HTTP / SOCKS5h URL，留空保留已保存代理" :disabled="saving" />
      </BaseFormItem>
      <span v-if="state?.proxyConfigured" class="break-all text-cp-xs text-cp-text-secondary">已配置：{{ state.proxyEndpoint }}</span>
      <fieldset class="m-0 grid max-h-60 grid-cols-1 gap-2 overflow-auto border-0 p-0 sm:grid-cols-2">
        <legend class="mb-2 font-bold">
          参与打票的账号
        </legend>
        <BaseCheckbox
          v-for="account in accounts"
          :key="account.id" show-label :model-value="selected.includes(account.id)"
          :label="account.name + (account.enabled ? '' : '（调度已停止）')" :disabled="saving"
          @update:model-value="select(account.id, $event)"
        >
          {{ account.name }}{{ account.enabled ? '' : '（调度已停止）' }}
        </BaseCheckbox>
      </fieldset>
      <div class="flex items-center justify-between border-t border-cp-border-secondary pt-3">
        <span class="font-bold">门票状态 · 近 1 小时</span>
        <BaseIconButton label="刷新门票状态" :loading="loading" @click="load(!initialized)">
          <RefreshCw class="size-4" />
        </BaseIconButton>
      </div>
      <div v-for="account in state?.accounts" :key="account.accountId" class="border-t border-cp-border-secondary pt-3">
        <div class="mb-2 text-cp font-bold">
          {{ account.name }} <span v-if="!account.enabled" class="font-normal text-cp-text-secondary">调度已停止</span>
        </div>
        <div v-for="model in account.models" :key="model.model" class="mb-3 grid gap-1 text-cp-xs">
          <div class="flex flex-wrap justify-between gap-2">
            <span>{{ model.model }}</span>
            <span :class="model.ready ? 'text-cp-success-text' : 'text-cp-warning-text'">{{ model.ready ? `剩余 ${Math.floor(model.remainingSeconds / 60)} 分钟` : model.blocked ? '缺票，模型暂停' : '暂无有效门票' }}</span>
          </div>
          <div class="text-cp-text-secondary">
            {{ model.uniqueIps }} IP · {{ model.attempts }} 次尝试 · 成功率 {{ model.attempts ? `${(100 * model.successes / model.attempts).toFixed(1)}%` : '-' }}
            <span v-if="model.unknownIpAttempts"> · {{ model.unknownIpAttempts }} 次 IP 未知</span>
          </div>
          <div class="text-cp-text-tertiary">
            {{ date(model.lastAttempt?.at) }} · HTTP {{ model.lastAttempt?.status || '-' }} · {{ model.lastAttempt?.length ?? 0 }} B
          </div>
        </div>
      </div>
    </div>
    <template #footer>
      <BaseButton variant="secondary" :disabled="saving" @click="open = false">
        关闭
      </BaseButton>
      <BaseButton variant="primary" :loading="saving" :disabled="loading || !state || !initialized" @click="save()">
        保存打票设置
      </BaseButton>
    </template>
  </BaseModal>
</template>
