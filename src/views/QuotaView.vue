<script setup lang="ts">
import type { AccountSummary, CodexState } from "../types";
import {
  accountStatusClass,
  accountStatusLabel,
  formatDate,
  usageClass,
  usageLabel,
  usagePercentLabel,
  usageResetLabel,
  usageWindowClass,
  usageWindowPercentClass,
  usageWindowWidth
} from "../utils/format";

defineProps<{
  accounts: AccountSummary[];
  selectedQuotaAccountId: string;
  selectedQuotaAccount: AccountSummary | null;
  current: CodexState | null;
  busy: boolean;
  isOperationActive: (key: string) => boolean;
  hasActiveOperation: boolean;
}>();

defineEmits<{
  "update:selectedQuotaAccountId": [string];
  refreshAll: [];
  fetchUsage: [AccountSummary | null];
  fetchAllUsage: [];
  refreshTokens: [AccountSummary];
  clearUsage: [AccountSummary];
}>();
</script>

<template>
  <section class="panel quota-page">
    <div class="panel-head">
      <div>
        <strong>Codex 额度监测</strong>
        <p>所有账号平铺展示，手动检查 usage 状态，不做后台轮询。</p>
      </div>
      <div class="actions">
        <button class="secondary" :disabled="busy" @click="$emit('refreshAll')">刷新列表</button>
        <button
          :disabled="busy || !accounts.length || hasActiveOperation || isOperationActive('quota:all')"
          @click="$emit('fetchAllUsage')"
        >
          检查全部
        </button>
      </div>
    </div>

    <div v-if="!accounts.length" class="empty compact-empty">
      <strong>没有账号</strong>
      <p>先导入或登录 Codex 账号，再回到这里检查额度。</p>
    </div>

    <div v-else class="quota-card-grid">
      <article
        v-for="account in accounts"
        :key="account.id"
        :class="['quota-account-card', { selected: selectedQuotaAccountId === account.id }]"
        @click="$emit('update:selectedQuotaAccountId', account.id)"
      >
        <div class="quota-card-head">
          <div class="provider-avatar compact" aria-hidden="true">C</div>
          <div class="quota-card-title">
            <div class="card-badge-row">
              <span class="type-badge codex-type">Codex</span>
              <span :class="['state-badge', accountStatusClass(account, current)]">
                {{ accountStatusLabel(account, current) }}
              </span>
              <span :class="['quota-percent', usageClass(account.usage_state)]">
                {{ usageLabel(account.usage_state) }}
              </span>
            </div>
            <h3>{{ account.display_name }}</h3>
            <p>{{ account.email || account.account_id || "无账号标识" }}</p>
          </div>
        </div>

        <dl class="account-meta quota-card-meta">
          <div>
            <dt>套餐</dt>
            <dd class="premium-plan-value">
              {{ account.usage_state?.raw_plan_type || account.plan || "Codex" }}
            </dd>
          </div>
          <div>
            <dt>最近检查</dt>
            <dd>{{ formatDate(account.usage_state?.last_checked_at) }}</dd>
          </div>
          <div>
            <dt>恢复时间</dt>
            <dd>{{ account.usage_state?.resets_at ? formatDate(account.usage_state.resets_at) : "-" }}</dd>
          </div>
          <div>
            <dt>HTTP</dt>
            <dd>{{ account.usage_state?.http_status || "-" }}</dd>
          </div>
        </dl>

        <div v-if="account.usage_state?.last_error" class="health-status-message quota-error-message">
          {{ account.usage_state.last_error }}
        </div>

        <div v-if="account.usage_state?.windows?.length" class="usage-window-list compact-usage-list">
          <div v-for="window in account.usage_state.windows" :key="window.id" class="usage-window-row">
            <div class="quota-row">
              <div class="quota-title">
                <span class="quota-model">{{ window.label }}</span>
                <span :class="['quota-percent', usageWindowPercentClass(window)]">
                  {{ usagePercentLabel(window) }}
                </span>
              </div>
              <span class="quota-reset">{{ usageResetLabel(window) }}</span>
            </div>
            <div class="quota-bar" aria-hidden="true">
              <div :class="usageWindowClass(window)" :style="{ width: usageWindowWidth(window) }"></div>
            </div>
          </div>
        </div>

        <div v-else class="quota-card-empty">
          还没有额度数据。点击“检查额度”后会用该账号 token 查询 Codex usage 状态。
        </div>

        <div class="quota-card-actions">
          <button
            class="secondary"
            :disabled="busy || isOperationActive('quota:all') || isOperationActive('refresh-token:' + account.id)"
            @click.stop="$emit('refreshTokens', account)"
          >
            刷新 token
          </button>
          <button
            :disabled="busy || isOperationActive('quota:all') || isOperationActive('quota:' + account.id)"
            @click.stop="$emit('fetchUsage', account)"
          >
            检查额度
          </button>
          <button
            class="secondary"
            :disabled="busy || isOperationActive('quota:all') || !account.usage_state || isOperationActive('quota:' + account.id)"
            @click.stop="$emit('clearUsage', account)"
          >
            清除记录
          </button>
        </div>
      </article>
    </div>
  </section>
</template>
