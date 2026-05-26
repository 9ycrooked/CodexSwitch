<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { computed, onMounted, reactive, ref } from "vue";
import type {
  AccountSummary,
  AppPaths,
  AutoFlowOAuthServerStatus,
  BackupSummary,
  CodexState,
  NetworkExitCheckResult,
  Settings
} from "./types";
import * as api from "./api/codexSwitchApi";
import {
  openAppStoreDir,
  openBackupDir,
  openBackupsDir,
  openBrowserProfileDir,
  openCodexHomeDir,
  readAppPaths
} from "./api/codexSwitchApi";
import AppSidebar from "./components/AppSidebar.vue";
import AppTitlebar from "./components/AppTitlebar.vue";
import DeleteAccountDialog from "./components/DeleteAccountDialog.vue";
import ToastViewport from "./components/ToastViewport.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import { useAccounts } from "./composables/useAccounts";
import { useBackups } from "./composables/useBackups";
import { useNotifications, type ToastType } from "./composables/useNotifications";
import { useQuota } from "./composables/useQuota";
import { useUpdater } from "./composables/useUpdater";
import { useWindowControls } from "./composables/useWindowControls";
import AccountsView from "./views/AccountsView.vue";
import BackupsView from "./views/BackupsView.vue";
import QuotaView from "./views/QuotaView.vue";
import SettingsView from "./views/SettingsView.vue";

type Tab = "accounts" | "quota" | "backups" | "settings";

const accounts = ref<AccountSummary[]>([]);
const backups = ref<BackupSummary[]>([]);
const current = ref<CodexState | null>(null);
const selectedTab = ref<"accounts" | "quota" | "backups" | "settings">("accounts");
const query = ref("");
const busy = ref(false);
const activeOperation = ref("");
const appVersion = ref("");
const appPaths = ref<AppPaths | null>(null);
const networkCheckResult = ref<NetworkExitCheckResult | null>(null);
const networkCheckRunning = ref(false);
const autoFlowServerStatus = ref<AutoFlowOAuthServerStatus | null>(null);
const autoFlowServerBusy = ref(false);
const deleteAccountTarget = ref<AccountSummary | null>(null);
const deleteAccountProfile = ref(false);
const exportDialogOpen = ref(false);
const selectedExportAccountIds = ref<string[]>([]);
const SUPPORT_EMAIL = "qianmang1@gmail.com";

const settings = reactive<Settings>({
  codex_home: "C:\\Users\\Y\\.codex",
  process_names: ["Codex.exe", "codex.exe"],
  browser_profile_dir: "",
  oauth_callback_port: 1455,
  keep_login_profiles: true,
  oauth_login_mode: "external",
  check_updates_on_startup: true,
  force_update_on_startup: false,
  manual_update_check_only: false,
  check_oauth_network_on_login: true,
  check_egress_region: false,
  autoflow_oauth_server_enabled: false,
  autoflow_oauth_server_port: 8080,
  autoflow_oauth_admin_key: ""
});
const savedSettingsSnapshot = ref(settingsSnapshot(settings));

const notifications = useNotifications();

const filteredAccounts = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return accounts.value;
  return accounts.value.filter((account) => {
    return [account.display_name, account.email, account.account_id, account.plan]
      .filter(Boolean)
      .some((value) => String(value).toLowerCase().includes(needle));
  });
});
const hasActiveOperation = computed(() => Boolean(activeOperation.value));
const showTopbarRefresh = computed(() => selectedTab.value === "accounts" || selectedTab.value === "backups");
const topbarRefreshLabel = computed(() => (selectedTab.value === "backups" ? "刷新备份" : "刷新账号"));
const allExportAccountsSelected = computed(
  () => Boolean(accounts.value.length) && selectedExportAccountIds.value.length === accounts.value.length
);
const settingsDirty = computed(() => {
  return savedSettingsSnapshot.value !== "" && savedSettingsSnapshot.value !== settingsSnapshot(settings);
});

function settingsSnapshot(value: Settings) {
  return JSON.stringify({
    codex_home: value.codex_home,
    process_names: value.process_names,
    browser_profile_dir: value.browser_profile_dir,
    oauth_callback_port: value.oauth_callback_port,
    keep_login_profiles: value.keep_login_profiles,
    oauth_login_mode: value.oauth_login_mode,
    check_updates_on_startup: value.check_updates_on_startup,
    force_update_on_startup: value.force_update_on_startup,
    manual_update_check_only: value.manual_update_check_only,
    check_oauth_network_on_login: value.check_oauth_network_on_login,
    check_egress_region: value.check_egress_region,
    autoflow_oauth_server_enabled: value.autoflow_oauth_server_enabled,
    autoflow_oauth_server_port: value.autoflow_oauth_server_port,
    autoflow_oauth_admin_key: value.autoflow_oauth_admin_key
  });
}

function applySettings(nextSettings: Settings) {
  Object.assign(settings, nextSettings);
  savedSettingsSnapshot.value = settingsSnapshot(nextSettings);
}

function applySettingsFromAsync(
  nextSettings: Settings,
  startedWithSnapshot: string,
  fieldsToMirror: Array<keyof Settings> = []
) {
  if (settingsSnapshot(settings) === startedWithSnapshot) {
    applySettings(nextSettings);
    return;
  }
  savedSettingsSnapshot.value = settingsSnapshot(nextSettings);
  for (const field of fieldsToMirror) {
    settings[field] = nextSettings[field] as never;
  }
}

function isOperationActive(key: string) {
  return activeOperation.value === key;
}

function updateProcessNames(event: Event) {
  settings.process_names = (event.target as HTMLInputElement).value.split(",");
}

function eventChecked(event: Event) {
  return (event.target as HTMLInputElement).checked;
}

function setMessage(type: ToastType, message: string) {
  notifications[type](message);
}

async function refreshAll() {
  const [nextAccounts, nextBackups, nextCurrent, nextSettings, nextAutoFlowStatus] = await Promise.all([
    api.listAccounts(),
    api.listBackups(),
    api.readCurrentCodexState(),
    api.readSettings(),
    api.getAutoFlowOAuthServerStatus()
  ]);
  accounts.value = nextAccounts;
  backups.value = nextBackups;
  current.value = nextCurrent;
  if (!settingsDirty.value) {
    applySettings(nextSettings);
  }
  autoFlowServerStatus.value = nextAutoFlowStatus;
}

async function refreshAppInfo() {
  const [version, paths] = await Promise.all([getVersion(), readAppPaths()]);
  appVersion.value = version;
  appPaths.value = paths;
}

async function refreshAllWithBusy() {
  busy.value = true;
  try {
    await refreshAll();
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    busy.value = false;
  }
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
  lastUpdateCheckedAt,
  runUpdateCheck,
  checkForUpdatesManually,
  installPendingUpdate,
  dismissUpdateDialog
} = useUpdater(settings, setMessage);

const {
  chooseAndImport,
  exportSelectedAccounts,
  exportSelectedAuthJsonOnly,
  startOAuthLogin,
  closeOAuthLogin,
  refreshTokens,
  reloginAccount,
  deleteStoredAccount,
  switchAccount
} = useAccounts({
  accounts,
  current,
  settings,
  activeOperation,
  refreshAll,
  setMessage
});

const {
  selectedQuotaAccountId,
  selectedQuotaAccount,
  selectQuotaAccount: selectQuotaAccountBase,
  fetchUsage,
  fetchAllUsage,
  clearUsage
} = useQuota({
  accounts,
  activeOperation,
  refreshAll,
  setMessage
});

const { createBackup, restoreBackup } = useBackups({
  backups,
  activeOperation,
  refreshAll,
  setMessage
});

function selectQuotaAccount(account?: AccountSummary | null) {
  selectQuotaAccountBase(account);
  selectedTab.value = "quota";
}

function selectTab(tab: Tab) {
  if (tab === "quota") {
    selectQuotaAccount();
    return;
  }
  selectedTab.value = tab;
}

function requestDeleteAccount(account: AccountSummary) {
  deleteAccountTarget.value = account;
  deleteAccountProfile.value = false;
}

function cancelDeleteAccount() {
  if (deleteAccountTarget.value && isOperationActive(`delete:${deleteAccountTarget.value.id}`)) return;
  deleteAccountTarget.value = null;
  deleteAccountProfile.value = false;
}

function openExportDialog() {
  selectedExportAccountIds.value = accounts.value.map((account) => account.id);
  exportDialogOpen.value = true;
}

function cancelExportDialog() {
  if (isOperationActive("accounts:export") || isOperationActive("accounts:export-auth")) return;
  exportDialogOpen.value = false;
}

function toggleExportAccount(accountId: string, checked: boolean) {
  const selected = new Set(selectedExportAccountIds.value);
  if (checked) {
    selected.add(accountId);
  } else {
    selected.delete(accountId);
  }
  selectedExportAccountIds.value = Array.from(selected);
}

function toggleAllExportAccounts(checked: boolean) {
  selectedExportAccountIds.value = checked ? accounts.value.map((account) => account.id) : [];
}

async function confirmExportAccounts() {
  const exported = await exportSelectedAccounts(selectedExportAccountIds.value);
  if (exported) {
    exportDialogOpen.value = false;
  }
}

async function confirmExportSelectedAuthJson() {
  const exported = await exportSelectedAuthJsonOnly(selectedExportAccountIds.value);
  if (exported) {
    exportDialogOpen.value = false;
  }
}

async function confirmDeleteAccount() {
  const account = deleteAccountTarget.value;
  if (!account) return;
  const deleted = await deleteStoredAccount(account, deleteAccountProfile.value);
  if (deleted) {
    deleteAccountTarget.value = null;
    deleteAccountProfile.value = false;
  }
}

async function saveSettings() {
  const operationKey = "settings:save";
  activeOperation.value = operationKey;
  try {
    const processNames = settings.process_names
      .flatMap((item) => item.split(","))
      .map((item) => item.trim())
      .filter(Boolean);
    const saved = await api.updateSettings({
      codex_home: settings.codex_home,
      process_names: processNames,
      browser_profile_dir: settings.browser_profile_dir,
      oauth_callback_port: Number(settings.oauth_callback_port),
      keep_login_profiles: Boolean(settings.keep_login_profiles),
      oauth_login_mode: settings.oauth_login_mode,
      check_updates_on_startup: settings.check_updates_on_startup,
      force_update_on_startup: settings.force_update_on_startup,
      manual_update_check_only: settings.manual_update_check_only,
      check_oauth_network_on_login: settings.check_oauth_network_on_login,
      check_egress_region: settings.check_egress_region,
      autoflow_oauth_server_enabled: Boolean(settings.autoflow_oauth_server_enabled),
      autoflow_oauth_server_port: Number(settings.autoflow_oauth_server_port),
      autoflow_oauth_admin_key: settings.autoflow_oauth_admin_key
    });
    applySettings(saved);
    await refreshAll();
    try {
      await refreshAppInfo();
      setMessage("success", "设置已保存");
    } catch (err) {
      setMessage("warning", `设置已保存，但应用信息刷新失败：${String(err)}`);
    }
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    if (activeOperation.value === operationKey) activeOperation.value = "";
  }
}

async function checkNetworkExitManually() {
  networkCheckRunning.value = true;
  try {
    const result = await api.checkOauthNetworkExit(settings.check_egress_region);
    networkCheckResult.value = result;
    const toastType: ToastType =
      result.overall_status === "ok" ? "success" : result.overall_status === "warning" ? "warning" : "error";
    setMessage(toastType, "登录前网络检查完成");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    networkCheckRunning.value = false;
  }
}

async function startAutoFlowServer() {
  autoFlowServerBusy.value = true;
  const startedWithSnapshot = settingsSnapshot(settings);
  try {
    const saved = await api.updateSettings({
      ...settings,
      autoflow_oauth_server_port: Number(settings.autoflow_oauth_server_port)
    });
    applySettingsFromAsync(saved, startedWithSnapshot, [
      "autoflow_oauth_admin_key",
      "autoflow_oauth_server_enabled"
    ]);
    autoFlowServerStatus.value = await api.startAutoFlowOAuthServer();
    const refreshed = await api.readSettings();
    applySettingsFromAsync(refreshed, startedWithSnapshot, [
      "autoflow_oauth_admin_key",
      "autoflow_oauth_server_enabled"
    ]);
    setMessage("success", "AutoFlow 接入服务已开启");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    autoFlowServerBusy.value = false;
  }
}

async function stopAutoFlowServer() {
  autoFlowServerBusy.value = true;
  const startedWithSnapshot = settingsSnapshot(settings);
  try {
    autoFlowServerStatus.value = await api.stopAutoFlowOAuthServer();
    const refreshed = await api.readSettings();
    applySettingsFromAsync(refreshed, startedWithSnapshot, [
      "autoflow_oauth_admin_key",
      "autoflow_oauth_server_enabled"
    ]);
    setMessage("success", "AutoFlow 接入服务已关闭");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    autoFlowServerBusy.value = false;
  }
}

async function resetAutoFlowAdminKey() {
  autoFlowServerBusy.value = true;
  const startedWithSnapshot = settingsSnapshot(settings);
  try {
    const saved = await api.resetAutoFlowOAuthAdminKey();
    applySettingsFromAsync(saved, startedWithSnapshot, ["autoflow_oauth_admin_key"]);
    autoFlowServerStatus.value = await api.getAutoFlowOAuthServerStatus();
    setMessage("success", "AutoFlow 管理密钥已重置");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    autoFlowServerBusy.value = false;
  }
}

async function copyAutoFlowText(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    setMessage("success", `${label}已复制`);
  } catch (err) {
    setMessage("error", `复制失败：${String(err)}`);
  }
}

async function runOpenDirectory(key: string, action: () => Promise<unknown>) {
  const operationKey = `open:${key}`;
  activeOperation.value = operationKey;
  try {
    await action();
    setMessage("info", "已打开目录");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    if (activeOperation.value === operationKey) activeOperation.value = "";
  }
}

const openCodexHome = () => runOpenDirectory("codex-home", openCodexHomeDir);
const openAppData = () => runOpenDirectory("app-data", openAppStoreDir);
const openProfiles = () => runOpenDirectory("profiles", openBrowserProfileDir);
const openBackups = () => runOpenDirectory("backups", openBackupsDir);
const openBackup = (backup: BackupSummary) => runOpenDirectory(`backup:${backup.id}`, () => openBackupDir(backup.id));

async function runExternalAction(key: string, label: string, action: () => Promise<unknown>) {
  const operationKey = `external:${key}`;
  activeOperation.value = operationKey;
  try {
    await action();
    setMessage("info", label);
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    if (activeOperation.value === operationKey) activeOperation.value = "";
  }
}

const openProjectRepository = () =>
  runExternalAction("repository", "已打开项目仓库", api.openProjectRepository);
const openProjectIssues = () =>
  runExternalAction("issues", "已打开 Issue 页面", api.openProjectIssues);
const openSupportEmail = () =>
  runExternalAction("support-email", "已打开邮件应用", api.openSupportEmail);

async function copySupportEmail() {
  try {
    await navigator.clipboard.writeText(SUPPORT_EMAIL);
    setMessage("success", "邮箱已复制");
  } catch (err) {
    setMessage("error", `复制失败：${String(err)}`);
  }
}

onMounted(async () => {
  busy.value = true;
  try {
    await refreshAll();
    try {
      await refreshAppInfo();
    } catch (err) {
      setMessage("error", String(err));
    }
    if (!selectedQuotaAccountId.value && accounts.value[0]) {
      selectedQuotaAccountId.value = accounts.value[0].id;
    }
    void runUpdateCheck();
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    busy.value = false;
  }
});
</script>

<template>
  <main class="shell">
    <AppTitlebar
      :on-minimize="minimizeWindow"
      :on-toggle-maximize="toggleMaximizeWindow"
      :on-close="closeWindow"
      :on-start-drag="startWindowDrag"
      :on-double-click="handleTitlebarDoubleClick"
    />

    <AppSidebar :selected-tab="selectedTab" :current="current" @select="selectTab" />

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
          <button v-if="showTopbarRefresh" class="secondary" :disabled="busy" @click="refreshAllWithBusy">
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M21 12a9 9 0 1 1-2.64-6.36" />
              <path d="M21 3v6h-6" />
            </svg>
            <span>{{ topbarRefreshLabel }}</span>
          </button>
          <button
            v-if="selectedTab === 'accounts'"
            :disabled="busy || isOperationActive('accounts:import')"
            @click="chooseAndImport"
          >
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3v12" />
              <path d="m7 10 5 5 5-5" />
              <path d="M5 21h14" />
            </svg>
            <span>批量导入</span>
          </button>
          <button
            v-if="selectedTab === 'accounts'"
            class="secondary"
            :disabled="busy || !accounts.length || isOperationActive('accounts:export')"
            @click="openExportDialog"
          >
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 21V9" />
              <path d="m7 14 5-5 5 5" />
              <path d="M5 3h14" />
            </svg>
            <span>导出账号</span>
          </button>
          <button
            v-if="selectedTab === 'accounts'"
            :disabled="busy || isOperationActive('oauth:start')"
            @click="startOAuthLogin"
          >
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M15 7h1a5 5 0 0 1 0 10h-1" />
              <path d="M9 17H8A5 5 0 0 1 8 7h1" />
              <path d="M8 12h8" />
            </svg>
            <span>OAuth 登录</span>
          </button>
          <button
            v-if="selectedTab === 'backups'"
            :disabled="busy || isOperationActive('backup:create')"
            @click="createBackup"
          >
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3v12" />
              <path d="m17 10-5 5-5-5" />
              <path d="M5 21h14" />
            </svg>
            <span>立即备份</span>
          </button>
        </div>
      </div>

      <AccountsView
        v-if="selectedTab === 'accounts'"
        v-model:query="query"
        :accounts="accounts"
        :filtered-accounts="filteredAccounts"
        :current="current"
        :busy="busy"
        :is-operation-active="isOperationActive"
        :has-active-operation="hasActiveOperation"
        @refresh-tokens="refreshTokens"
        @select-quota-account="selectQuotaAccount"
        @relogin-account="reloginAccount"
        @delete-account="requestDeleteAccount"
      />

      <QuotaView
        v-else-if="selectedTab === 'quota'"
        v-model:selectedQuotaAccountId="selectedQuotaAccountId"
        :accounts="accounts"
        :selected-quota-account="selectedQuotaAccount"
        :current="current"
        :busy="busy"
        :is-operation-active="isOperationActive"
        :has-active-operation="hasActiveOperation"
        @refresh-all="refreshAllWithBusy"
        @fetch-usage="fetchUsage"
        @fetch-all-usage="fetchAllUsage"
        @refresh-tokens="refreshTokens"
        @clear-usage="clearUsage"
        @switch-account="switchAccount"
      />

      <BackupsView
        v-else-if="selectedTab === 'backups'"
        :backups="backups"
        :busy="busy"
        :is-operation-active="isOperationActive"
        :has-active-operation="hasActiveOperation"
        @restore-backup="restoreBackup"
        @open-backups="openBackups"
        @open-backup="openBackup"
      />

      <SettingsView
        v-else
        :settings="settings"
        :app-version="appVersion"
        :app-paths="appPaths"
        :last-update-checked-at="lastUpdateCheckedAt"
        :busy="busy || isOperationActive('settings:save')"
        :settings-dirty="settingsDirty"
        :update-checking="updateChecking"
        :update-downloading="updateDownloading"
        :update-policy-source="updatePolicySource"
        :update-policy-error="updatePolicyError"
        :network-check-result="networkCheckResult"
        :network-check-running="networkCheckRunning"
        :autoflow-server-status="autoFlowServerStatus"
        :autoflow-server-busy="autoFlowServerBusy"
        @update-process-names="updateProcessNames"
        @check-for-updates="checkForUpdatesManually"
        @check-network-exit="checkNetworkExitManually"
        @start-autoflow-server="startAutoFlowServer"
        @stop-autoflow-server="stopAutoFlowServer"
        @reset-autoflow-admin-key="resetAutoFlowAdminKey"
        @copy-autoflow-service-url="
          copyAutoFlowText(
            autoFlowServerStatus?.url || `http://127.0.0.1:${settings.autoflow_oauth_server_port}/admin/accounts`,
            'AutoFlow 地址'
          )
        "
        @copy-autoflow-admin-key="copyAutoFlowText(settings.autoflow_oauth_admin_key, '管理密钥')"
        @save-settings="saveSettings"
        @open-codex-home="openCodexHome"
        @open-app-data="openAppData"
        @open-profiles="openProfiles"
        @open-project-repository="openProjectRepository"
        @open-project-issues="openProjectIssues"
        @copy-support-email="copySupportEmail"
        @open-support-email="openSupportEmail"
      />
    </section>

    <ToastViewport
      :toasts="notifications.toasts.value"
      @close="notifications.remove"
      @pause="notifications.pause"
      @resume="notifications.resume"
    />

    <UpdateDialog
      :open="updateDialogOpen"
      :forced="updateIsForced"
      :downloading="updateDownloading"
      :error="updateError"
      :policy-message="updatePolicy.message"
      :current-version="pendingUpdateInfo?.currentVersion"
      :next-version="pendingUpdateInfo?.version"
      :notes="pendingUpdateNotes"
      :progress-percent="updateProgressPercent"
      :has-total-bytes="Boolean(updateTotalBytes)"
      @dismiss="dismissUpdateDialog"
      @install="installPendingUpdate"
      @close-app="closeWindow"
    />

    <DeleteAccountDialog
      v-model:delete-profile="deleteAccountProfile"
      :open="Boolean(deleteAccountTarget)"
      :account="deleteAccountTarget"
      :deleting="Boolean(deleteAccountTarget && isOperationActive(`delete:${deleteAccountTarget.id}`))"
      @cancel="cancelDeleteAccount"
      @confirm="confirmDeleteAccount"
    />

    <div v-if="exportDialogOpen" class="modal-backdrop">
      <section class="modal export-account-modal" role="dialog" aria-modal="true" aria-labelledby="export-account-title">
        <div class="modal-header">
          <div>
            <p class="eyebrow">账号包</p>
            <h2 id="export-account-title">导出账号和登录环境</h2>
          </div>
          <button
            class="modal-close"
            type="button"
            aria-label="关闭导出账号"
            :disabled="isOperationActive('accounts:export') || isOperationActive('accounts:export-auth')"
            @click="cancelExportDialog"
          >
            ×
          </button>
        </div>

        <div class="export-account-copy">
          <p>导出的 ZIP 会包含所选账号的 auth.json，以及该账号绑定的浏览器 Profile（cookie、缓存和本地存储）。</p>
          <p>这个文件等同于登录凭据，请只在自己的设备之间迁移。</p>
        </div>

        <label class="checkbox-row export-select-all">
          <input
            type="checkbox"
            :checked="allExportAccountsSelected"
            :disabled="isOperationActive('accounts:export') || isOperationActive('accounts:export-auth')"
            @change="toggleAllExportAccounts(eventChecked($event))"
          />
          <span>全选账号</span>
        </label>

        <div class="export-account-list">
          <label v-for="account in accounts" :key="account.id" class="export-account-row">
            <input
              type="checkbox"
              :checked="selectedExportAccountIds.includes(account.id)"
              :disabled="isOperationActive('accounts:export') || isOperationActive('accounts:export-auth')"
              @change="toggleExportAccount(account.id, eventChecked($event))"
            />
            <span>
              <strong>{{ account.display_name }}</strong>
              <small>{{ account.email || account.account_id || account.id }}</small>
            </span>
            <em>{{ account.browser_profile_dir ? "含登录环境" : "仅凭据" }}</em>
          </label>
        </div>

        <div class="modal-actions">
          <button
            class="secondary"
            type="button"
            :disabled="
              isOperationActive('accounts:export') ||
              isOperationActive('accounts:export-auth') ||
              !selectedExportAccountIds.length
            "
            @click="confirmExportSelectedAuthJson"
          >
            {{ isOperationActive("accounts:export-auth") ? "正在导出" : "仅导出 auth.json" }}
          </button>
          <button
            class="secondary"
            type="button"
            :disabled="isOperationActive('accounts:export') || isOperationActive('accounts:export-auth')"
            @click="cancelExportDialog"
          >
            取消
          </button>
          <button
            type="button"
            :disabled="
              isOperationActive('accounts:export') ||
              isOperationActive('accounts:export-auth') ||
              !selectedExportAccountIds.length
            "
            @click="confirmExportAccounts"
          >
            {{ isOperationActive("accounts:export") ? "正在导出" : `导出 ${selectedExportAccountIds.length} 个账号` }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>
