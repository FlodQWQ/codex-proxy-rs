<script setup lang="ts">
import type { Account } from '@/api'
import type { ModelFingerprintModel, ModelFingerprintTestResult } from '@/api/modules/model-fingerprint'
import { Fingerprint, RefreshCw } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { getAccounts } from '@/api'
import { getFingerprintModels, testAccountFingerprint } from '@/api/modules/model-fingerprint'
import BaseButton from '@/components/base/BaseButton.vue'
import BaseCheckbox from '@/components/base/BaseCheckbox.vue'
import BaseFormItem from '@/components/base/BaseForm/FormItem.vue'
import BaseModal from '@/components/base/BaseModal/index.vue'
import BaseSelect from '@/components/base/BaseSelect.vue'

type FingerprintResult = ModelFingerprintTestResult & { error?: string }

const emit = defineEmits<{ completed: [] }>()
const open = defineModel<boolean>({ required: true })
const loading = ref(false)
const running = ref(false)
const loadError = ref('')
const accounts = ref<Account[]>([])
const models = ref<ModelFingerprintModel[]>([])
const selectedAccountIds = ref<string[]>([])
const selectedModel = ref('')
const results = ref<FingerprintResult[]>([])
const currentIndex = ref(0)
const selectedAccounts = computed(() => accounts.value.filter(account => selectedAccountIds.value.includes(account.id)))
const modelOptions = computed(() => models.value.map(model => ({ label: model.label, value: model.id })))

async function load() {
  if (loading.value || running.value)
    return
  loading.value = true
  loadError.value = ''
  try {
    const first = await getAccounts({ page: 1, pageSize: 200, provider: 'openai' })
    const rows = [...first.items]
    for (let page = 2; page <= first.page.totalPages; page++) {
      const next = await getAccounts({ page, pageSize: 200, provider: 'openai' })
      rows.push(...next.items)
    }
    accounts.value = rows
    const catalog = await getFingerprintModels()
    models.value = catalog.models
    if (!models.value.some(model => model.id === selectedModel.value))
      selectedModel.value = models.value[0]?.id ?? ''
  }
  catch {
    loadError.value = '账号或 VPS 本地指纹库读取失败'
  }
  finally {
    loading.value = false
  }
}

function selectAccount(id: string, value: boolean) {
  selectedAccountIds.value = value
    ? [...new Set([...selectedAccountIds.value, id])]
    : selectedAccountIds.value.filter(item => item !== id)
}

function selectAll(value: boolean) {
  selectedAccountIds.value = value ? accounts.value.map(account => account.id) : []
}

function setResult(accountId: string, result: FingerprintResult) {
  results.value = [...results.value.filter(item => item.accountId !== accountId), result]
}

async function runTests() {
  if (running.value || !selectedModel.value || selectedAccounts.value.length === 0)
    return
  running.value = true
  results.value = []
  currentIndex.value = 0
  let completed = false
  for (const [index, account] of selectedAccounts.value.entries()) {
    currentIndex.value = index + 1
    try {
      const result = await testAccountFingerprint(
        { accountId: account.id, modelId: selectedModel.value },
        { timeout: 600_000, silent: true },
      )
      setResult(account.id, result)
      completed = true
    }
    catch (error: unknown) {
      setResult(account.id, {
        accountId: account.id,
        sentModel: selectedModel.value,
        responseModel: null,
        confidence: null,
        status: 'inconclusive',
        attempted: 0,
        usedOutputs: 0,
        expiresAt: null,
        error: error instanceof Error ? error.message : '指纹检测失败',
      })
    }
  }
  running.value = false
  currentIndex.value = 0
  if (completed)
    emit('completed')
}

watch(open, (value) => {
  if (value) {
    results.value = []
    void load()
  }
})

function confidence(value: number | null) {
  return value === null ? '-' : `${(value * 100).toFixed(1)}%`
}
</script>

<template>
  <BaseModal v-model="open" title="模型指纹检测" size="lg" :dismissible="!running">
    <div class="grid gap-4">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="m-0 text-cp-sm text-cp-text-secondary">
          {{ loading ? '正在加载账号和本地指纹库（首次可能需要几分钟）…' : '3 条有效回答 · 置信度至少 70%' }}
        </p>
        <BaseButton variant="secondary" :disabled="loading || running" @click="load()">
          <RefreshCw class="size-4" :class="loading ? 'animate-spin motion-reduce:animate-none' : ''" />刷新列表
        </BaseButton>
      </div>

      <div v-if="loadError" role="alert" class="text-cp-error-text">
        {{ loadError }}
      </div>

      <BaseFormItem label="目标模型">
        <BaseSelect
          v-model="selectedModel"
          aria-label="目标模型"
          :options="modelOptions"
          :disabled="loading || running || models.length === 0"
          placeholder="选择要验证的模型"
          empty-text="没有可比较的 GPT 模型"
        />
      </BaseFormItem>

      <fieldset class="m-0 grid gap-2 border-0 p-0">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <legend class="font-bold">
            测试账号 · {{ selectedAccountIds.length }} / {{ accounts.length }}
          </legend>
          <div class="flex gap-2">
            <BaseButton variant="secondary" size="sm" :disabled="loading || running || accounts.length === 0" @click="selectAll(true)">
              全选
            </BaseButton>
            <BaseButton variant="secondary" size="sm" :disabled="running || selectedAccountIds.length === 0" @click="selectAll(false)">
              清空
            </BaseButton>
          </div>
        </div>
        <div class="grid max-h-56 grid-cols-1 gap-2 overflow-y-auto sm:grid-cols-2">
          <BaseCheckbox
            v-for="account in accounts"
            :key="account.id"
            show-label
            :model-value="selectedAccountIds.includes(account.id)"
            :label="`${account.name}${account.enabled ? '' : '（调度已停止）'}`"
            :disabled="loading || running"
            @update:model-value="selectAccount(account.id, $event)"
          >
            {{ account.name }}{{ account.enabled ? '' : '（调度已停止）' }}
          </BaseCheckbox>
          <p v-if="!loading && accounts.length === 0 && !loadError" class="m-0 text-cp-sm text-cp-text-tertiary">
            没有 OpenAI 账号
          </p>
        </div>
      </fieldset>

      <section v-if="running" role="status" aria-live="polite" class="flex items-center gap-2 border-t border-cp-border-secondary pt-3 text-cp-info-text">
        <RefreshCw class="size-4 animate-spin motion-reduce:animate-none" />
        正在检测第 {{ currentIndex }} / {{ selectedAccountIds.length }} 个账号
      </section>

      <section v-if="results.length" aria-label="指纹检测结果" class="grid gap-2 border-t border-cp-border-secondary pt-3">
        <div v-for="result in results" :key="result.accountId" class="grid gap-1 border-b border-cp-border-secondary pb-2 text-cp-sm last:border-0">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="font-bold">{{ accounts.find(account => account.id === result.accountId)?.name || result.accountId }}</span>
            <span
              class="font-bold"
              :class="result.error || result.status === 'degraded' ? 'text-cp-error-text' : result.status === 'passed' ? 'text-cp-success-text' : 'text-cp-warning-text'"
            >
              {{ result.error ? '检测失败' : result.status === 'degraded' ? '已降智' : result.status === 'passed' ? '未检测到降智' : '证据不足' }}
            </span>
          </div>
          <span v-if="result.error" class="text-cp-text-secondary">{{ result.error }}</span>
          <span v-else-if="result.responseModel" class="break-all text-cp-text-secondary">
            {{ result.sentModel }} → {{ result.responseModel }} · 置信度 {{ confidence(result.confidence) }}
          </span>
          <span v-else class="text-cp-text-secondary">
            有效回答 {{ result.usedOutputs }} / 3 · 尝试 {{ result.attempted }} 题，未生成标记
          </span>
          <span v-if="result.expiresAt && result.status === 'degraded'" class="text-cp-xs text-cp-text-tertiary">
            标记到期 {{ new Date(result.expiresAt).toLocaleString() }}
          </span>
        </div>
      </section>

      <p class="m-0 text-cp-xs text-cp-text-tertiary">
        每个账号最多请求 6 次 · 会消耗上游额度 · 模型归因仅供参考
      </p>
    </div>
    <template #footer>
      <BaseButton variant="secondary" :disabled="running" @click="open = false">
        关闭
      </BaseButton>
      <BaseButton
        variant="primary"
        :loading="running"
        :disabled="loading || running || !selectedModel || selectedAccountIds.length === 0"
        @click="runTests()"
      >
        <Fingerprint class="size-4" />开始检测
      </BaseButton>
    </template>
  </BaseModal>
</template>
