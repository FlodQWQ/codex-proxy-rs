import type { TicketSettings } from '@/api/modules/codex-tickets'
import { useIntervalFn } from '@vueuse/core'
import { computed, onMounted, onScopeDispose, shallowRef } from 'vue'
import { getCodexTickets } from '@/api/modules/codex-tickets'

export function useCodexTicketStatus() {
  const state = shallowRef<TicketSettings | null>(null)
  const error = shallowRef(false)
  let pending: AbortController | undefined
  async function reload() {
    if (pending)
      return
    const controller = new AbortController()
    pending = controller
    try {
      const next = await getCodexTickets({ silent: true, signal: controller.signal })
      if (!controller.signal.aborted) {
        state.value = next
        error.value = false
      }
    }
    catch {
      if (!controller.signal.aborted)
        error.value = true
    }
    finally {
      pending = undefined
    }
  }
  onMounted(reload)
  onScopeDispose(() => pending?.abort())
  useIntervalFn(() => {
    if (document.visibilityState === 'visible')
      void reload()
  }, 10_000)
  return {
    ticketAccounts: computed(() => new Map(state.value?.accounts.map(account => [account.accountId, account]) ?? [])),
    ticketsEnabled: computed(() => state.value?.enabled ?? false),
    ticketStatusError: error,
    reloadTicketStatus: reload,
  }
}
