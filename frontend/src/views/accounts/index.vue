<script setup lang="ts">
import { ChevronDown, Ticket } from '@lucide/vue'
import { ref } from 'vue'

import BaseButton from '@/components/base/BaseButton.vue'
import BaseCheckbox from '@/components/base/BaseCheckbox.vue'
import BaseConfirmModal from '@/components/base/BaseConfirmModal.vue'
import BasePageHeader from '@/components/base/BasePageHeader.vue'
import BaseSwitch from '@/components/base/BaseSwitch.vue'
import BaseTableColumnSettings from '@/components/base/BaseTable/BaseTableColumnSettings.vue'
import BaseTablePagination from '@/components/base/BaseTable/BaseTablePagination.vue'
import BaseTable from '@/components/base/BaseTable/index.vue'
import { useTableColumns } from '@/components/base/BaseTable/useTableColumns'
import LastUsedAtCell from '@/components/LastUsedAtCell.vue'
import ProviderIconGroup from '@/components/ProviderIconGroup.vue'
import { useAccountGroupCatalog } from '@/composables/useAccountGroupCatalog'
import AccountBatchEditModal from './components/AccountBatchEditModal.vue'
import AccountConnectionTestModal from './components/AccountConnectionTestModal.vue'
import AccountCreateModal from './components/AccountCreateModal/index.vue'
import AccountEditModal from './components/AccountEditModal.vue'
import AccountFilters from './components/AccountFilters.vue'
import AccountImportTasks from './components/AccountImportTasks/index.vue'
import AccountOverviewCards from './components/AccountOverviewCards.vue'
import AccountPlanBadge from './components/AccountPlanBadge.vue'
import AccountQuotaPanel from './components/AccountQuotaPanel/index.vue'
import AccountStatusBadge from './components/AccountStatusBadge/index.vue'
import AccountTableActions from './components/AccountTableActions.vue'
import AccountTableIdentity from './components/AccountTableIdentity.vue'
import AccountTableUsage from './components/AccountTableUsage.vue'
import AccountTicketStatus from './components/AccountTicketStatus.vue'
import AccountUsagePanel from './components/AccountUsagePanel.vue'
import CodexTicketsModal from './components/CodexTicketsModal.vue'
import { useAccountBatchEditor } from './composables/useAccountBatchEditor'
import { useAccountConnectionTest } from './composables/useAccountConnectionTest'
import { useAccountEditor } from './composables/useAccountEditor'
import { useAccountImportTasks } from './composables/useAccountImportTasks'
import { useAccountMutations } from './composables/useAccountMutations'
import { useAccountsQuery } from './composables/useAccountsQuery'
import { useAccountsTable } from './composables/useAccountsTable'
import { useCodexTicketStatus } from './composables/useCodexTicketStatus'
import { accountColumns, derivedAccountStatus } from './constants'

const selectedIds = ref<Set<string>>(new Set())
const showCodexTickets = ref(false)
const { ticketAccounts, ticketsEnabled, ticketStatusError, reloadTicketStatus } = useCodexTicketStatus()
const { visibleColumns, columnOptions, setColumnVisible, setColumnOrder, resetColumns } = useTableColumns(accountColumns, 'accounts')
const {
  loading,
  accounts,
  loadAccounts,
  refreshAccountsSilently,
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
} = useAccountsQuery()

const {
  groups,
  loading: groupsLoading,
  loadGroups,
} = useAccountGroupCatalog()

const importTasks = useAccountImportTasks({
  reload: () => Promise.all([loadAccounts(), loadGroups()]),
})
const {
  open: showImportTasks,
  tasks: recentImportTasks,
  selectedId: importTaskId,
  detail: importTaskDetail,
  loading: loadingImportTasks,
  stopping: stoppingImportTask,
  error: importTaskError,
  activeCount: activeImportCount,
} = importTasks

const {
  showCreateModal,
  showDeleteModal,
  showSingleDeleteModal,
  pendingDeleteAccount,
  recoveringAccountIds,
  refreshingAccountIds,
  refreshingQuotaAccountIds,
  deletingAccount,
  creatingAccount,
  authorizingOAuth,
  batchDeleting,
  exportingAccounts,
  reauthorizingAccount,
  createForm,
  handleCreate,
  handleAuthorizeOAuth,
  openCreateAccount,
  openReauthorizeAccount,
  requestDeleteAccount,
  handleDelete,
  handleBatchDelete,
  handleExportAccounts,
  handleRecover,
  handleRefresh,
  handleRefreshQuota,
  schedulingAccountIds,
  handleToggleScheduling,
} = useAccountMutations({
  onImportTaskCreated: importTasks.created,
  accounts,
  selectedIds,
  reload: () => Promise.all([loadAccounts(), loadGroups()]),
  replaceAccount,
})

const {
  showConnectionTestModal,
  testingAccount,
  connectionTestStatus,
  connectionTestModel,
  connectionTestLogs,
  connectionTestError,
  connectionTestStartedAt,
  connectionTestFinishedAt,
  connectionTestDurationMs,
  testingConnectionIds,
  loadingConnectionTestModels,
  refreshingConnectionTestModels,
  connectionTestSelectedModel,
  connectionTestModelOptions,
  connectionTestStatusView,
  openConnectionTest,
  handleRefreshConnectionTestModels,
  handleTestConnection,
} = useAccountConnectionTest({ reload: refreshAccountsSilently })

const {
  expandedAccountIds,
  allSelected,
  indeterminate,
  selectedRowKeys,
  expandedRowKeys,
  toggleSelection,
  toggleExpanded,
  toggleAll,
} = useAccountsTable(accounts, selectedIds)

const {
  showBatchEditModal,
  schedulingEnabled: batchSchedulingEnabled,
  concurrencyLimit: batchConcurrencyLimit,
  weight: batchWeight,
  modelAccess: batchModelAccess,
  hasChanges: batchHasChanges,
  catalogAccountId: batchCatalogAccountId,
  proxyMode: batchProxyMode,
  proxyId: batchProxyId,
  selectedGroupIds: batchGroupIds,
  saving: savingBatchEdit,
  open: openBatchEdit,
  save: saveBatchEdit,
} = useAccountBatchEditor({
  accounts,
  selectedIds,
  reloadAccounts: loadAccounts,
  reloadGroups: loadGroups,
})

const {
  apiKey: editingApiKey,
  configurationLoading,
  configurationReady,
  showEditModal,
  editingAccount,
  notes: editingNotes,
  schedulingEnabled,
  concurrencyLimit: editingConcurrencyLimit,
  weight: editingWeight,
  modelAccess: editingModelAccess,
  proxyMode: editingProxyMode,
  proxyId: editingProxyId,
  selectedGroupIds: editingGroupIds,
  saving: savingAccountEdit,
  open: openAccountEdit,
  save: saveAccountEdit,
} = useAccountEditor({
  accounts,
  reloadAccounts: loadAccounts,
  reloadGroups: loadGroups,
})
</script>

<template>
  <div class="flex min-h-0 w-full flex-col gap-3 xl:h-full xl:overflow-hidden">
    <BasePageHeader
      class="min-h-12! [&_h1]:text-2xl"
      title="账号管理"
    />

    <AccountOverviewCards :summary="accountSummary" />

    <section
      class="flex min-h-0 flex-1 flex-col"
    >
      <div class="shrink-0 pb-3">
        <AccountFilters
          v-model:search="searchQuery"
          v-model:status="statusQuery"
          v-model:provider="providerQuery"
          v-model:group="groupQuery"
          :groups="groups"
          :groups-loading="groupsLoading"
          :loading="loading || refreshingQuotas"
          :selected-count="selectedIds.size"
          :batch-deleting="batchDeleting"
          :exporting-accounts="exportingAccounts"
          :has-import-tasks="recentImportTasks.length > 0"
          :active-import-count="activeImportCount"
          @import-tasks="showImportTasks = true"
          @delete-selected="showDeleteModal = true"
          @export-selected="handleExportAccounts"
          @create="openCreateAccount"
          @edit-selected="openBatchEdit"
          @refresh="refreshAccountsWithQuota()"
        >
          <template #actions>
            <BaseButton variant="secondary" @click="showCodexTickets = true">
              <Ticket class="size-4" />292 打票
            </BaseButton>
            <span v-if="ticketStatusError" role="status" class="text-cp-xs text-cp-warning-text">打票状态读取失败</span>
            <BaseTableColumnSettings
              :options="columnOptions"
              @change="setColumnVisible"
              @reorder="setColumnOrder"
              @reset="resetColumns"
            />
          </template>
        </AccountFilters>
      </div>

      <div class="flex min-h-0 flex-1 flex-col">
        <div class="flex min-h-0 flex-col xl:h-full">
          <BaseTable
            class="h-[60dvh]! min-h-80 flex-none [--cp-table-row-height-sm:72px] xl:h-auto! xl:min-h-0 xl:flex-1"
            density="compact"
            :columns="visibleColumns"
            :rows="accounts"
            :loading="loading"
            :selected-row-keys="selectedRowKeys"
            :expanded-row-keys="expandedRowKeys"
            :sort="sort"
            empty-text="暂无账号数据"
            @sort-change="handleSortChange"
          >
            <template #expander="{ row }">
              <button
                type="button"
                class="inline-flex size-6 cursor-pointer items-center justify-center rounded-md border-0 bg-transparent text-cp-text-secondary transition hover:bg-cp-bg-text-hover hover:text-cp-text"
                :title="expandedAccountIds.has(row.id) ? '收起统计' : '展开统计'"
                @click.stop="toggleExpanded(row.id)"
              >
                <ChevronDown
                  class="size-3.5 transition-transform"
                  :class="expandedAccountIds.has(row.id) ? '' : '-rotate-90'"
                />
              </button>
            </template>

            <template #header-selection>
              <BaseCheckbox
                :model-value="allSelected"
                :indeterminate="indeterminate"
                label="选择当前页账号"
                @update:model-value="toggleAll"
              />
            </template>

            <template #selection="{ row }">
              <BaseCheckbox
                :model-value="selectedIds.has(row.id)"
                label="选择账号"
                @update:model-value="toggleSelection(row.id)"
              />
            </template>

            <template #identity="{ row }">
              <AccountTableIdentity :account="row" @edit="openAccountEdit" />
            </template>

            <template #provider="{ row }">
              <div class="flex flex-col items-center gap-1.5">
                <ProviderIconGroup :provider="row.provider" :authentication-kind="row.authenticationKind" />
                <AccountPlanBadge :authentication-kind="row.authenticationKind" :plan-type="row.planType" :plan-type-display="row.planTypeDisplay" />
              </div>
            </template>

            <template #status="{ row }">
              <AccountStatusBadge
                :status="derivedAccountStatus(row)"
                :error-reason="row.errorReason"
                :error-message="row.errorMessage"
                :rate-limited-until="row.quota.rateLimitedUntil"
                :rate-limit-reason="row.quota.rateLimitReason"
                :recovery-probe-required="row.quota.recoveryProbeRequired"
                :next-refresh-at="row.nextRefreshAt"
              />
            </template>

            <template #planType="{ row }">
              <AccountPlanBadge :authentication-kind="row.authenticationKind" :plan-type="row.planType" :plan-type-display="row.planTypeDisplay" />
            </template>

            <template #usage="{ row }">
              <AccountTicketStatus
                v-if="ticketAccounts.has(row.id)" :account="ticketAccounts.get(row.id)!"
                :enabled="ticketsEnabled" :error="ticketStatusError"
              />
              <AccountTableUsage
                :account="row"
                :refreshing="refreshingQuotaAccountIds.has(row.id) || refreshingQuotas"
                @refresh-quota="handleRefreshQuota"
              />
            </template>

            <template #scheduling="{ row }">
              <div class="flex flex-col items-center gap-1">
                <BaseSwitch
                  :key="`${row.id}:${schedulingAccountIds.has(row.id)}`"
                  :model-value="row.enabled"
                  :label="`允许调度 ${row.name}`"
                  :disabled="schedulingAccountIds.has(row.id)"
                  @update:model-value="handleToggleScheduling(row)"
                />
                <span class="text-cp-xs" :class="row.enabled ? 'text-cp-success-text' : 'text-cp-text-tertiary'">
                  {{ schedulingAccountIds.has(row.id) ? '保存中' : row.enabled ? '已开启' : '已停止' }}
                </span>
              </div>
            </template>

            <template #concurrency="{ row }">
              <div class="flex flex-col gap-1 text-center text-cp-xs tabular-nums">
                <span :title="`并发上限：${row.concurrencyLimit ?? '继承系统'}`">{{ row.concurrencyLimit ?? '继承' }}</span>
                <span class="text-cp-text-tertiary">权重 {{ row.weight }}</span>
              </div>
            </template>

            <template #outboundProxyEndpoint="{ row }">
              <span class="block truncate text-cp-xs" :title="row.outboundProxyEndpoint ?? '直连'">{{ row.outboundProxyEndpoint ?? '直连' }}</span>
            </template>

            <template #groups="{ row }">
              <div class="flex min-w-0 flex-wrap gap-1">
                <span
                  v-for="group in row.groups" :key="group.id"
                  class="max-w-full truncate rounded border border-cp-border-secondary bg-cp-fill-quaternary px-1.5 py-0.5 text-cp-xs"
                  :class="group.enabled ? 'text-cp-text-secondary' : 'text-cp-text-disabled'"
                  :title="group.name + (group.enabled ? '' : '（已禁用）')"
                >
                  {{ group.name }}{{ group.enabled ? '' : '（停用）' }}
                </span>
                <span v-if="!row.groups.length" class="text-cp-xs text-cp-text-tertiary">未分组</span>
              </div>
            </template>

            <template #lastUsedAt="{ row }">
              <LastUsedAtCell :value="row.usage.lastUsedAt" />
            </template>

            <template #actions="{ row }">
              <AccountTableActions
                :account="row"
                :deleting="deletingAccount"
                :recovering="recoveringAccountIds.has(row.id)"
                :refreshing="refreshingAccountIds.has(row.id)"
                :testing="testingConnectionIds.has(row.id)"
                @edit="openAccountEdit"
                @delete="requestDeleteAccount"
                @recover="handleRecover"
                @refresh="handleRefresh"
                @reauthorize="openReauthorizeAccount"
                @test="openConnectionTest"
              />
            </template>

            <template #expanded="{ row }">
              <div class="grid items-stretch gap-3 p-4 lg:grid-cols-[1.05fr_2.45fr] xl:min-h-77">
                <AccountQuotaPanel
                  :account="row"
                  :refreshing="refreshingQuotaAccountIds.has(row.id)"
                  @account-updated="void replaceAccount($event)"
                  @refresh-quota="handleRefreshQuota"
                />
                <AccountUsagePanel
                  :account="row"
                  @account-updated="void replaceAccount($event)"
                />
              </div>
            </template>
          </BaseTable>
          <BaseTablePagination
            :pagination="accountPagination"
            :loading="loading"
            @page-change="handlePageChange"
            @page-size-change="handlePageSizeChange"
          />
        </div>
      </div>
    </section>

    <CodexTicketsModal v-model="showCodexTickets" @saved="reloadTicketStatus" />
    <AccountConnectionTestModal
      v-model="showConnectionTestModal"
      v-model:selected-model="connectionTestSelectedModel"
      :account="testingAccount"
      :duration-ms="connectionTestDurationMs"
      :error="connectionTestError"
      :finished-at="connectionTestFinishedAt"
      :loading-models="loadingConnectionTestModels"
      :refreshing-models="refreshingConnectionTestModels"
      :logs="connectionTestLogs"
      :model="connectionTestModel"
      :model-options="connectionTestModelOptions"
      :started-at="connectionTestStartedAt"
      :status="connectionTestStatus"
      :status-view="connectionTestStatusView"
      @refresh-models="handleRefreshConnectionTestModels()"
      @test="handleTestConnection()"
    />

    <AccountImportTasks
      v-model="showImportTasks"
      :tasks="recentImportTasks"
      :selected-id="importTaskId"
      :detail="importTaskDetail"
      :loading="loadingImportTasks"
      :stopping="stoppingImportTask"
      :error="importTaskError"
      @select="importTasks.select"
      @refresh="importTasks.refresh"
      @stop="importTasks.stop"
      @view-accounts="showImportTasks = false; loadAccounts()"
    />

    <AccountCreateModal
      v-model="showCreateModal"
      v-model:form="createForm"
      :account="reauthorizingAccount"
      :groups="groups"
      :groups-loading="groupsLoading"
      :oauth-loading="authorizingOAuth"
      :reauthorizing="Boolean(reauthorizingAccount)"
      :saving="creatingAccount"
      @create="handleCreate"
      @generate-oauth="handleAuthorizeOAuth"
    />

    <AccountEditModal
      v-model="showEditModal"
      v-model:api-key="editingApiKey"
      v-model:notes="editingNotes"
      v-model:enabled="schedulingEnabled"
      v-model:concurrency-limit="editingConcurrencyLimit"
      v-model:weight="editingWeight"
      v-model:model-access="editingModelAccess"
      v-model:proxy-mode="editingProxyMode"
      v-model:proxy-id="editingProxyId"
      v-model:selected-group-ids="editingGroupIds"
      :configuration-loading="configurationLoading"
      :configuration-ready="configurationReady"
      :account="editingAccount"
      :groups="groups"
      :groups-loading="groupsLoading"
      :saving="savingAccountEdit"
      @save="saveAccountEdit"
    />

    <AccountBatchEditModal
      v-model="showBatchEditModal"
      v-model:enabled="batchSchedulingEnabled"
      v-model:concurrency-limit="batchConcurrencyLimit"
      v-model:weight="batchWeight"
      v-model:model-access="batchModelAccess"
      v-model:proxy-mode="batchProxyMode"
      v-model:proxy-id="batchProxyId"
      v-model:selected-group-ids="batchGroupIds"
      :catalog-account-id="batchCatalogAccountId"
      :selected-count="selectedIds.size"
      :groups="groups"
      :groups-loading="groupsLoading"
      :saving="savingBatchEdit"
      :has-changes="batchHasChanges"
      @save="saveBatchEdit"
    />

    <BaseConfirmModal
      v-model="showDeleteModal"
      title="确认删除"
      description="删除后该账号将不再参与调度，此操作不可撤销"
      destructive
      confirm-text="确认删除"
      :loading="batchDeleting"
      @confirm="handleBatchDelete"
    >
      <p class="m-0">
        确定要删除选中的 {{ selectedIds.size }} 个账号吗？此操作不可撤销
      </p>
    </BaseConfirmModal>

    <BaseConfirmModal
      v-model="showSingleDeleteModal"
      title="删除账号"
      description="删除后该账号将不再参与调度，此操作不可撤销"
      destructive
      confirm-text="确认删除"
      :loading="deletingAccount"
      @confirm="handleDelete"
    >
      <p class="m-0">
        确定要删除
        {{
          pendingDeleteAccount?.email
            || pendingDeleteAccount?.accountId
            || pendingDeleteAccount?.id
            || '该账号'
        }}
        吗？
      </p>
    </BaseConfirmModal>
  </div>
</template>
