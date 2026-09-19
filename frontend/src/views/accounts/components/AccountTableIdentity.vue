<script setup lang="ts">
import type { AccountRow } from '../constants'
import { computed } from 'vue'
import { accountTableName, accountTableNotes } from '../utils/tableIdentity'

const props = defineProps<{ account: AccountRow }>()
const emit = defineEmits<{ edit: [account: AccountRow] }>()
const title = computed(() => accountTableName(props.account))
const notes = computed(() => accountTableNotes(props.account))
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
  </div>
</template>
