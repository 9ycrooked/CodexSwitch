<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { computed, onMounted, reactive, ref } from "vue";
import type { AccountSummary, AppPaths, BackupSummary, CodexState, NetworkExitCheckResult, Settings } from "./types";
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

const settings = reactive<Settings>({
  codex_home: "C:\\Users\\Y\\.codex",
  process_names: ["Codex.exe", "codex.exe"],
  close_timeout_ms: 6000,
  browser_profile_dir: "",
  oauth_callback_port: 1455,
  keep_login_profiles: true,
  oauth_login_mode: "external",
  check_updates_on_startup: true,
  force_update_on_startup: false,
  check_oauth_network_on_login: true,
  check_egress_region: false,
  autoflow_oauth_server_enabled: false,
  autoflow_oauth_server_port: 8080,
  autoflow_oauth_admin_key: ""
});

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

function isOperationActive(key: string) {
  return activeOperation.value === key;
}

function updateProcessNames(event: Event) {
  settings.process_names = (event.target as HTMLInputElement).value.split(",");
}

function setMessage(type: ToastType, message: string) {
  notifications[type](message);
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

const { chooseAndImport, startOAuthLogin, closeOAuthLogin, refreshTokens, switchAccount } = useAccounts({
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
      close_timeout_ms: Number(settings.close_timeout_ms),
      browser_profile_dir: settings.browser_profile_dir,
      oauth_callback_port: Number(settings.oauth_callback_port),
      keep_login_profiles: Boolean(settings.keep_login_profiles),
      oauth_login_mode: settings.oauth_login_mode,
      check_updates_on_startup: settings.check_updates_on_startup,
      force_update_on_startup: settings.force_update_on_startup,
      check_oauth_network_on_login: settings.check_oauth_network_on_login,
      check_egress_region: settings.check_egress_region,
      autoflow_oauth_server_enabled: Boolean(settings.autoflow_oauth_server_enabled),
      autoflow_oauth_server_port: Number(settings.autoflow_oauth_server_port),
      autoflow_oauth_admin_key: settings.autoflow_oauth_admin_key
    });
    Object.assign(settings, saved);
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
          <button class="secondary" :disabled="busy" @click="refreshAllWithBusy">
            <svg class="button-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M21 12a9 9 0 1 1-2.64-6.36" />
              <path d="M21 3v6h-6" />
            </svg>
            <span>刷新</span>
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
        @switch-account="switchAccount"
        @refresh-tokens="refreshTokens"
        @select-quota-account="selectQuotaAccount"
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
        :update-checking="updateChecking"
        :update-downloading="updateDownloading"
        :update-policy-source="updatePolicySource"
        :update-policy-error="updatePolicyError"
        :network-check-result="networkCheckResult"
        :network-check-running="networkCheckRunning"
        @update-process-names="updateProcessNames"
        @check-for-updates="checkForUpdatesManually"
        @check-network-exit="checkNetworkExitManually"
        @save-settings="saveSettings"
        @open-codex-home="openCodexHome"
        @open-app-data="openAppData"
        @open-profiles="openProfiles"
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
  </main>
</template>
