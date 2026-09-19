<script setup lang="ts">
import type { AccountRow } from '../constants'
import { useNow } from '@vueuse/core'
import { computed } from 'vue'
import { accountTableName, accountTableNotes } from '../utils/tableIdentity'

const props = defineProps<{ account: AccountRow }>()
const emit = defineEmits<{ edit: [account: AccountRow] }>()
const title = computed(() => accountTableName(props.account))
const notes = computed(() => accountTableNotes(props.account))
const now = useNow({ interval: 1000 })
const markers = computed(() => (props.account.modelDegradation ?? []).filter(item => Date.parse(item.expiresAt) > now.value.getTime()))
const degraded = computed(() => markers.value.some(item => item.status === 'degraded'))
const degradationTitle = computed(() => markers.value.map(item =>
  `${item.status === 'degraded' ? '已降智' : '降智已缓解'}：${item.sentModel} → ${item.responseModel}\n最近降智：${item.detectedAt}\n${item.recoveredAt ? `恢复观测：${item.recoveredAt}\n` : ''}标记到期：${item.expiresAt}\n路由：${item.routingScope} ${item.groupIds.join(', ')}`,
).join('\n\n'))
</script>

<template>
  <div class="flex min-w-0 flex-col gap-1 py-1">
    <button
      type="button"
      class="max-w-full cursor-pointer truncate border-0 bg-transparent p-0 text-left text-cp font-bold text-cp-text hover:text-cp-link focus-visible:outline-2 focus-visible:outline-cp-control-outline"
      :title="title"
      :aria-label="`编辑账号 ${title}`"
      @click.stop="emit('edit', account)"
    >
      {{ title }}
    </button>
    <span v-if="account.email && account.email !== title" class="truncate text-cp-xs text-cp-text-secondary" :title="account.email">
      {{ account.email }}
    </span>
    <span v-if="notes" class="truncate text-cp-xs text-cp-text-tertiary" :title="notes">
      {{ notes }}
    </span>
    <span
      v-if="markers.length" class="text-cp-xs font-bold"
      :class="degraded ? 'text-cp-error' : 'text-cp-warning'" :title="degradationTitle"
      tabindex="0" :aria-label="degradationTitle"
    >
      {{ degraded ? '已降智' : '降智已缓解' }}
    </span>
  </div>
</template>
