<script setup lang="ts">
import type { AccountSummary, CodexState } from "../types";
import {
  accountStatusClass,
  accountStatusLabel,
  formatDate,
  isCurrentAccount,
  quotaClass,
  quotaLabel,
  quotaTimestamp
} from "../utils/format";

defineProps<{
  accounts: AccountSummary[];
  filteredAccounts: AccountSummary[];
  current: CodexState | null;
  busy: boolean;
  isOperationActive: (key: string) => boolean;
  hasActiveOperation: boolean;
  query: string;
}>();

defineEmits<{
  "update:query": [string];
  refreshTokens: [AccountSummary];
  selectQuotaAccount: [AccountSummary];
  reloginAccount: [AccountSummary];
  deleteAccount: [AccountSummary];
}>();

function inputValue(event: Event) {
  return (event.target as HTMLInputElement).value;
}
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <input :value="query" placeholder="搜索 email、account id 或 plan" @input="$emit('update:query', inputValue($event))" />
      <span>{{ filteredAccounts.length }} / {{ accounts.length }} 个账号</span>
    </div>

    <div class="bundle-drop-hint">
      <strong>导入账号包</strong>
      <span>可直接把 Codex Switch 导出的 .zip 压缩包拖到窗口中，导入账号凭证和绑定的登录环境。</span>
    </div>

    <div v-if="!filteredAccounts.length" class="empty">
      <strong>还没有账号</strong>
      <p>导入 Codex Switch 账号压缩包，或用 OAuth 登录添加新账号。</p>
    </div>

    <div v-else class="account-grid">
      <article
        v-for="account in filteredAccounts"
        :key="account.id"
        :class="['account-card file-card', { current: isCurrentAccount(account, current), disabled: account.disabled }]"
      >
        <div class="file-card-layout">
          <div class="file-card-main">
            <div class="card-header">
              <div class="card-header-content">
                <div class="card-badge-row">
                  <span class="type-badge codex-type">Codex</span>
                  <span :class="['state-badge', accountStatusClass(account, current)]">
                    {{ accountStatusLabel(account, current) }}
                  </span>
                </div>
                <h3 class="file-name">{{ account.display_name }}</h3>
              </div>
            </div>

            <div v-if="account.quota_state?.last_error" class="health-status-message">
              {{ account.quota_state.last_error }}
            </div>

            <dl class="account-meta">
              <div>
                <dt>套餐</dt>
                <dd class="premium-plan-value">{{ account.plan || "Codex" }}</dd>
              </div>
              <div>
                <dt>过期</dt>
                <dd>{{ formatDate(account.expired) }}</dd>
              </div>
              <div>
                <dt>Config</dt>
                <dd>{{ account.has_config ? "已保存" : "无" }}</dd>
              </div>
              <div>
                <dt>Profile</dt>
                <dd>{{ account.browser_profile_dir ? "已隔离" : "未记录" }}</dd>
              </div>
            </dl>

            <div class="quota-section">
              <div class="quota-row">
                <div class="quota-title">
                  <span class="quota-model">额度状态</span>
                  <span :class="['quota-percent', quotaClass(account.quota_state)]">
                    {{ quotaLabel(account.quota_state) }}
                  </span>
                </div>
                <span class="quota-reset">{{ quotaTimestamp(account.quota_state) }}</span>
              </div>
            </div>

            <div class="card-actions">
              <button class="primary-action-button" :disabled="busy" @click="$emit('selectQuotaAccount', account)">
                额度
              </button>
              <button :disabled="busy || isOperationActive('refresh-token:' + account.id)" @click="$emit('refreshTokens', account)">
                刷新
              </button>
              <button :disabled="busy || isOperationActive('relogin:' + account.id)" @click="$emit('reloginAccount', account)">
                登录
              </button>
              <button
                class="secondary danger-button"
                :disabled="busy || isOperationActive('delete:' + account.id)"
                @click="$emit('deleteAccount', account)"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      </article>
    </div>
  </section>
</template>
