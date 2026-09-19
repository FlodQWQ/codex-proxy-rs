import type { BaseTableSort } from '@/components/base/BaseTable/columns'
import { watchDebounced } from '@vueuse/core'

import { computed, onMounted, onScopeDispose, shallowRef, watch } from 'vue'
import { getAccounts, refreshAccountQuota } from '@/api'
import { toast } from '@/components/base/BaseToast'
import { usePagedQuery } from '@/composables/usePagedQuery'

type AccountRow = Awaited<ReturnType<typeof getAccounts>>['items'][number]

export function useAccountsQuery() {
  const searchQuery = shallowRef('')
  const providerQuery = shallowRef('')
  const statusQuery = shallowRef('')
  const groupQuery = shallowRef('')
  const sort = shallowRef<BaseTableSort>()
  const refreshingQuotas = shallowRef(false)
  let quotaController: AbortController | undefined
  const accountSummary = shallowRef({
    total: 0,
    normal: 0,
    quotaExhausted: 0,
    rateLimited: 0,
    disabled: 0,
    error: 0,
  })

  const query = usePagedQuery({
    initialPageSize: 20,
    load: ({ page, pageSize }, options) =>
      getAccounts({
        page,
        pageSize,
        search: searchQuery.value,
        provider: providerQuery.value || undefined,
        status: statusQuery.value || undefined,
        groupId: groupQuery.value || undefined,
        sortBy: sort.value?.key,
        sortDirection: sort.value?.direction,
      }, options),
    onSuccess: (result) => {
      accountSummary.value = result.summary
    },
  })

  const accountPagination = computed(() => ({
    currentPage: query.page.value,
    pageSize: query.pageSize.value,
    total: query.total.value,
  }))

  function handlePageChange(page: number) {
    query.page.value = page
    void query.execute()
  }

  function handlePageSizeChange(pageSize: number) {
    query.pageSize.value = pageSize
    query.page.value = 1
    void query.execute()
  }

  function handleSortChange(nextSort: BaseTableSort | undefined) {
    sort.value = nextSort
    query.page.value = 1
    void query.execute()
  }

  async function replaceAccount(updated: AccountRow) {
    // 先取消旧查询并应用接口返回的账号，避免旧响应覆盖最新行数据。
    query.invalidate()
    query.items.value = query.items.value.map(account => account.id === updated.id ? updated : account)

    // 筛选、排序、概览和末页回退仍由回读校准，但不触发整表加载。
    if (!await query.execute({ background: true }))
      return true // 回读失败或被新查询取代时，不依据旧页面取消选择。
    return query.items.value.some(account => account.id === updated.id)
  }

  async function refreshAccountsWithQuota() {
    if (refreshingQuotas.value || query.loading.value)
      return
    refreshingQuotas.value = true
    const controller = new AbortController()
    quotaController = controller
    try {
      if (!await query.execute() || controller.signal.aborted)
        return
      const pending = query.items.value.filter(account => account.enabled && account.authenticationKind === 'oauth')
      const failed: string[] = []
      let succeeded = 0
      // 每次点击仅查询当页账号，并发最多为 2，不刷新令牌或发起推理。
      const worker = async () => {
        while (pending.length > 0 && !controller.signal.aborted) {
          const account = pending.shift()!
          try {
            await refreshAccountQuota({ accountId: account.id }, { silent: true, signal: controller.signal })
            succeeded += 1
          }
          catch {
            if (!controller.signal.aborted)
              failed.push(account.name)
          }
        }
      }
      await Promise.all([worker(), worker()])
      if (controller.signal.aborted)
        return
      if (!await query.execute({ background: true }) || controller.signal.aborted)
        return
      if (failed.length) {
        toast.warning(`配额查询成功 ${succeeded} 个，失败 ${failed.length} 个：${failed.join('、')}`, 8000)
      }
      else {
        toast.success(succeeded > 0 ? `列表及 ${succeeded} 个账号的上游配额已更新` : '列表已刷新，当前页没有已启用的 OAuth 账号')
      }
    }
    finally {
      if (quotaController === controller) {
        quotaController = undefined
        refreshingQuotas.value = false
      }
    }
  }

  onScopeDispose(() => quotaController?.abort())

  watchDebounced(
    searchQuery,
    () => {
      query.page.value = 1
      void query.execute()
    },
    { debounce: 250 },
  )

  watch([providerQuery, statusQuery, groupQuery], () => {
    query.page.value = 1
    void query.execute()
  })

  onMounted(() => {
    void query.execute()
  })

  return {
    page: query.page,
    pageSize: query.pageSize,
    totalAccounts: query.total,
    loading: query.loading,
    accounts: query.items,
    loadAccounts: query.execute,
    refreshAccountsSilently: () => query.execute({ silent: true }),
    refreshingQuotas,
    refreshAccountsWithQuota,
    searchQuery,
    providerQuery,
    statusQuery,
    groupQuery,
    sort,
    accountSummary,
    accountPagination,
    replaceAccount,
    handlePageChange,
    handlePageSizeChange,
    handleSortChange,
  }
}
