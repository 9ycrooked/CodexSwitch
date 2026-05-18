<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { Minus, Square, X } from "lucide-vue-next";
import type { AccountSummary, BackupSummary, CodexState, Settings } from "./types";
import * as api from "./api/codexSwitchApi";
import { useAccounts } from "./composables/useAccounts";
import { useBackups } from "./composables/useBackups";
import { useQuota } from "./composables/useQuota";
import { useUpdater } from "./composables/useUpdater";
import { useWindowControls } from "./composables/useWindowControls";
import {
  accountStatusClass,
  accountStatusLabel,
  formatDate,
  isCurrentAccount,
  quotaClass,
  quotaLabel,
  quotaTimestamp,
  usageClass,
  usageLabel,
  usagePercentLabel,
  usageResetLabel,
  usageWindowClass,
  usageWindowPercentClass,
  usageWindowWidth
} from "./utils/format";

const accounts = ref<AccountSummary[]>([]);
const backups = ref<BackupSummary[]>([]);
const current = ref<CodexState | null>(null);
const selectedTab = ref<"accounts" | "quota" | "backups" | "settings">("accounts");
const query = ref("");
const busy = ref(false);
const notice = ref("");
const error = ref("");

const settings = reactive<Settings>({
  codex_home: "C:\\Users\\Y\\.codex",
  process_names: ["Codex.exe", "codex.exe"],
  close_timeout_ms: 6000,
  browser_profile_dir: "",
  oauth_callback_port: 1455,
  keep_login_profiles: true,
  oauth_login_mode: "external",
  check_updates_on_startup: true,
  force_update_on_startup: false
});

const filteredAccounts = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return accounts.value;
  return accounts.value.filter((account) => {
    return [account.display_name, account.email, account.account_id, account.plan]
      .filter(Boolean)
      .some((value) => String(value).toLowerCase().includes(needle));
  });
});

function updateProcessNames(event: Event) {
  settings.process_names = (event.target as HTMLInputElement).value.split(",");
}

function setMessage(message: string, isError = false) {
  notice.value = isError ? "" : message;
  error.value = isError ? message : "";
}

async function refreshAll() {
  const [nextAccounts, nextBackups, nextCurrent, nextSettings] = await Promise.all([
    api.listAccounts(),
    api.listBackups(),
    api.readCurrentCodexState(),
    api.readSettings()
  ]);
  accounts.value = nextAccounts;
  backups.value = nextBackups;
  current.value = nextCurrent;
  Object.assign(settings, nextSettings);
}

const {
  minimizeWindow,
  toggleMaximizeWindow,
  closeWindow,
  startWindowDrag,
  handleTitlebarDoubleClick
} = useWindowControls();

const {
  updatePolicy,
  updatePolicySource,
  updatePolicyError,
  updateDialogOpen,
  updateChecking,
  updateDownloading,
  updateError,
  pendingUpdate,
  pendingUpdateInfo,
  pendingUpdateNotes,
  updateTotalBytes,
  updateProgressPercent,
  updateIsForced,
  runUpdateCheck,
  checkForUpdatesManually,
  installPendingUpdate,
  dismissUpdateDialog
} = useUpdater(settings, setMessage);

const { chooseAndImport, startOAuthLogin, closeOAuthLogin, refreshTokens, switchAccount } = useAccounts({
  accounts,
  current,
  busy,
  refreshAll,
  setMessage
});

const {
  selectedQuotaAccountId,
  selectedQuotaAccount,
  selectedUsageState,
  selectQuotaAccount: selectQuotaAccountBase,
  fetchUsage,
  clearUsage
} = useQuota({
  accounts,
  busy,
  refreshAll,
  setMessage
});

const { createBackup, restoreBackup } = useBackups({
  backups,
  busy,
  refreshAll,
  setMessage
});

function selectQuotaAccount(account?: AccountSummary | null) {
  selectQuotaAccountBase(account);
  selectedTab.value = "quota";
}

async function saveSettings() {
  busy.value = true;
  try {
    const processNames = settings.process_names
      .flatMap((item) => item.split(","))
      .map((item) => item.trim())
      .filter(Boolean);
    const saved = await api.updateSettings({
      codex_home: settings.codex_home,
      process_names: processNames,
      close_timeout_ms: Number(settings.close_timeout_ms),
      browser_profile_dir: settings.browser_profile_dir,
      oauth_callback_port: Number(settings.oauth_callback_port),
      keep_login_profiles: Boolean(settings.keep_login_profiles),
      oauth_login_mode: settings.oauth_login_mode,
      check_updates_on_startup: settings.check_updates_on_startup,
      force_update_on_startup: settings.force_update_on_startup
    });
    Object.assign(settings, saved);
    await refreshAll();
    setMessage("设置已保存。");
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  busy.value = true;
  try {
    await refreshAll();
    if (!selectedQuotaAccountId.value && accounts.value[0]) {
      selectedQuotaAccountId.value = accounts.value[0].id;
    }
    void runUpdateCheck();
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
});
</script>

<template>
  <main class="shell">
    <header class="app-titlebar">
      <div class="titlebar-brand" @mousedown="startWindowDrag" @dblclick="handleTitlebarDoubleClick">
        <span class="titlebar-mark" aria-hidden="true"></span>
        <span>Codex Switch</span>
      </div>
      <div class="titlebar-drag-region" @mousedown="startWindowDrag" @dblclick="handleTitlebarDoubleClick"></div>
      <div class="titlebar-window-controls">
        <button class="titlebar-button" type="button" aria-label="最小化" @click="minimizeWindow">
          <Minus :size="14" :stroke-width="1.8" aria-hidden="true" />
        </button>
        <button class="titlebar-button" type="button" aria-label="最大化或还原" @click="toggleMaximizeWindow">
          <Square :size="13" :stroke-width="1.8" aria-hidden="true" />
        </button>
        <button class="titlebar-button titlebar-close" type="button" aria-label="关闭" @click="closeWindow">
          <X :size="15" :stroke-width="1.8" aria-hidden="true" />
        </button>
      </div>
    </header>

    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">C</div>
        <div>
          <h1>Codex Switch</h1>
          <p>本地 OAuth 凭据管理</p>
        </div>
      </div>

      <nav class="tabs" aria-label="主导航">
        <button :class="{ active: selectedTab === 'accounts' }" @click="selectedTab = 'accounts'">
          <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z" />
            <path d="M4 21a8 8 0 0 1 16 0" />
          </svg>
          <span>账号</span>
        </button>
        <button :class="{ active: selectedTab === 'quota' }" @click="selectQuotaAccount()">
          <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 19V5" />
            <path d="M4 19h16" />
            <path d="M8 16v-5" />
            <path d="M12 16V8" />
            <path d="M16 16v-3" />
          </svg>
          <span>额度</span>
        </button>
        <button :class="{ active: selectedTab === 'backups' }" @click="selectedTab = 'backups'">
          <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 3v7l4 2" />
            <path d="M20 12a8 8 0 1 1-2.35-5.65" />
            <path d="M20 4v5h-5" />
          </svg>
          <span>备份</span>
        </button>
        <button :class="{ active: selectedTab === 'settings' }" @click="selectedTab = 'settings'">
          <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.05.05a2 2 0 0 1-2.83 2.83l-.05-.05a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.08a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.05.05a2 2 0 0 1-2.83-2.83l.05-.05A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.08a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.05-.05a2 2 0 0 1 2.83-2.83l.05.05A1.65 1.65 0 0 0 8.9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.08a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.05-.05a2 2 0 0 1 2.83 2.83l-.05.05A1.65 1.65 0 0 0 19.4 9c.14.47.51.84.98.98H21a2 2 0 0 1 0 4h-.08a1.65 1.65 0 0 0-1.52 1Z" />
          </svg>
          <span>设置</span>
        </button>
      </nav>

      <section class="status-panel">
        <span class="eyebrow">当前 Codex</span>
        <strong>{{ current?.current_email || current?.current_account_id || "未识别账号" }}</strong>
        <small>{{ current?.codex_home }}</small>
        <div class="state-row">
          <span :class="['dot', current?.auth_exists ? 'ok' : 'bad']"></span>
          auth.json
          <span :class="['dot', current?.config_exists ? 'ok' : 'bad']"></span>
          config.toml
        </div>
      </section>
    </aside>

    <section class="content">
      <div class="topbar">
        <div>
          <p class="eyebrow">Windows desktop app</p>
          <h2 v-if="selectedTab === 'accounts'">账号库</h2>
          <h2 v-else-if="selectedTab === 'quota'">额度监测</h2>
          <h2 v-else-if="selectedTab === 'backups'">备份记录</h2>
          <h2 v-else>设置</h2>
        </div>
        <div class="actions">
          <button class="secondary" :disabled="busy" @click="refreshAll">
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M21 12a9 9 0 1 1-2.64-6.36" />
              <path d="M21 3v6h-6" />
            </svg>
            <span>刷新</span>
          </button>
          <button v-if="selectedTab === 'accounts'" :disabled="busy" @click="chooseAndImport">
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3v12" />
              <path d="m7 10 5 5 5-5" />
              <path d="M5 21h14" />
            </svg>
            <span>批量导入</span>
          </button>
          <button v-if="selectedTab === 'accounts'" :disabled="busy" @click="startOAuthLogin">
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M15 7h1a5 5 0 0 1 0 10h-1" />
              <path d="M9 17H8A5 5 0 0 1 8 7h1" />
              <path d="M8 12h8" />
            </svg>
            <span>OAuth 登录</span>
          </button>
          <button v-if="selectedTab === 'backups'" :disabled="busy" @click="createBackup">
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3v12" />
              <path d="m17 10-5 5-5-5" />
              <path d="M5 21h14" />
            </svg>
            <span>立即备份</span>
          </button>
        </div>
      </div>

      <div v-if="notice" class="notice">{{ notice }}</div>
      <div v-if="error" class="notice error">{{ error }}</div>

      <section v-if="selectedTab === 'accounts'" class="panel">
        <div class="panel-head">
          <input v-model="query" placeholder="搜索 email、account id 或 plan" />
          <span>{{ filteredAccounts.length }} / {{ accounts.length }} 个账号</span>
        </div>

        <div v-if="!filteredAccounts.length" class="empty">
          <strong>还没有账号</strong>
          <p>导入 Codex OAuth JSON 或已有 auth.json，就能在这里一键切换。</p>
        </div>

        <div v-else class="account-grid">
          <article
            v-for="account in filteredAccounts"
            :key="account.id"
            :class="['account-card file-card', { current: isCurrentAccount(account, current), disabled: account.disabled }]"
          >
            <div class="file-card-layout">
              <div class="provider-avatar" aria-hidden="true">
                <span>C</span>
              </div>

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
                    <p class="file-description">{{ account.email || account.account_id || "无账号标识" }}</p>
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
                  <button class="primary-action-button" :disabled="busy || isCurrentAccount(account, current)" @click="switchAccount(account)">
                    切换
                  </button>
                  <button class="secondary" :disabled="busy" @click="refreshTokens(account)">刷新认证</button>
                  <button class="secondary" :disabled="busy" @click="selectQuotaAccount(account)">查看额度</button>
                </div>
              </div>
            </div>
          </article>
        </div>
      </section>

      <section v-else-if="selectedTab === 'quota'" class="panel quota-page">
        <div class="panel-head">
          <div>
            <strong>Codex 额度监测</strong>
            <p>所有账号平铺展示，手动检查 usage 状态，不做后台轮询。</p>
          </div>
          <div class="actions">
            <button class="secondary" :disabled="busy" @click="refreshAll">刷新列表</button>
            <button :disabled="busy || !selectedQuotaAccount" @click="fetchUsage(selectedQuotaAccount)">检查额度</button>
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
            @click="selectedQuotaAccountId = account.id"
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
              <button class="secondary" :disabled="busy" @click.stop="refreshTokens(account)">刷新 token</button>
              <button :disabled="busy" @click.stop="fetchUsage(account)">检查额度</button>
              <button class="secondary" :disabled="busy || !account.usage_state" @click.stop="clearUsage(account)">
                清除记录
              </button>
            </div>
          </article>
        </div>
      </section>

      <section v-else-if="selectedTab === 'backups'" class="panel">
        <div v-if="!backups.length" class="empty">
          <strong>暂无备份</strong>
          <p>每次切换账号前都会自动备份，也可以手动创建。</p>
        </div>
        <div v-else class="backup-list">
          <article v-for="backup in backups" :key="backup.id" class="backup-row card-row">
            <div>
              <h3>{{ backup.id }}</h3>
              <p>{{ formatDate(backup.created_at) }}</p>
              <small>
                auth: {{ backup.auth_path ? "有" : "无" }} · config:
                {{ backup.config_path ? "有" : "无" }}
              </small>
            </div>
            <button class="secondary" :disabled="busy" @click="restoreBackup(backup)">恢复</button>
          </article>
        </div>
      </section>

      <section v-else class="panel settings">
        <label class="form-group">
          <span>Codex home</span>
          <input v-model="settings.codex_home" />
        </label>
        <label class="form-group">
          <span>Codex 进程名</span>
          <input :value="settings.process_names.join(', ')" @input="updateProcessNames" />
        </label>
        <label class="form-group">
          <span>关闭超时（毫秒）</span>
          <input v-model.number="settings.close_timeout_ms" type="number" min="1000" step="500" />
        </label>
        <label class="form-group">
          <span>WebView2 Profile 目录</span>
          <input v-model="settings.browser_profile_dir" />
        </label>
        <label class="form-group">
          <span>OAuth callback 端口</span>
          <input v-model.number="settings.oauth_callback_port" type="number" min="1024" max="65535" />
        </label>
        <label class="form-group">
          <span>OAuth 登录方式</span>
          <select v-model="settings.oauth_login_mode">
            <option value="external">外部隔离浏览器（推荐）</option>
            <option value="embedded">内置 WebView2（实验）</option>
          </select>
        </label>
        <label class="checkbox-row">
          <input v-model="settings.keep_login_profiles" type="checkbox" />
          <span>保留登录 Profile，用于隔离并复用该账号的浏览器会话</span>
        </label>
        <section class="update-settings-panel">
          <div>
            <span class="eyebrow">Updater</span>
            <h3>更新检查</h3>
            <p>
              启动检查和强制更新策略由发布包里的 update-policy.json 控制。默认策略为启动时检查更新，发现新版本时询问是否更新。
            </p>
            <small>当前策略：{{ updatePolicySource }}</small>
            <small v-if="updatePolicyError">{{ updatePolicyError }}</small>
          </div>
          <button class="secondary" :disabled="busy || updateChecking || updateDownloading" @click="checkForUpdatesManually">
            {{ updateChecking ? "检查中" : "检查更新" }}
          </button>
        </section>
        <div class="warning">
          当前版本按你的要求使用明文保存账号凭据。WebView2 Profile 只是隔离 cookie/cache/localStorage，不伪造设备指纹；账号库包含 refresh token，请不要共享应用数据目录或导出的账号文件。
        </div>
        <button :disabled="busy" @click="saveSettings">保存设置</button>
      </section>
    </section>

    <div v-if="updateDialogOpen" class="modal-backdrop">
      <section class="modal update-modal" role="dialog" aria-modal="true" aria-labelledby="update-modal-title">
        <div class="modal-header">
          <div>
            <p class="eyebrow">软件更新</p>
            <h2 id="update-modal-title">发现新版本</h2>
          </div>
          <button
            v-if="!updateIsForced"
            class="modal-close"
            type="button"
            aria-label="关闭更新提示"
            :disabled="updateDownloading"
            @click="dismissUpdateDialog"
          >
            ×
          </button>
        </div>

        <p v-if="updatePolicy.message" class="update-policy-message">{{ updatePolicy.message }}</p>

        <div v-if="pendingUpdateInfo" class="update-version-row">
          <span>当前版本 {{ pendingUpdateInfo.currentVersion || "未知" }}</span>
          <span>新版本 {{ pendingUpdateInfo.version || "未知" }}</span>
        </div>

        <pre class="update-notes">{{ pendingUpdateNotes }}</pre>

        <div v-if="updateDownloading" class="update-progress">
          <div class="quota-bar" aria-hidden="true">
            <div
              class="quota-bar-fill quota-bar-fill-high"
              :style="{ width: updateTotalBytes ? `${updateProgressPercent}%` : '35%' }"
            ></div>
          </div>
          <p>{{ updateTotalBytes ? `${updateProgressPercent}%` : "正在下载更新..." }}</p>
        </div>

        <p v-if="updateError" class="notice error">{{ updateError }}</p>

        <div class="modal-actions">
          <button
            v-if="!updateIsForced"
            class="secondary"
            type="button"
            :disabled="updateDownloading"
            @click="dismissUpdateDialog"
          >
            稍后
          </button>
          <button
            v-else
            class="secondary"
            type="button"
            :disabled="updateDownloading"
            @click="closeWindow"
          >
            退出
          </button>
          <button type="button" :disabled="updateDownloading || !pendingUpdate" @click="installPendingUpdate">
            {{ updateDownloading ? "正在更新" : "立即更新" }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>
