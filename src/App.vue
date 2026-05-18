<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import type { AccountSummary, BackupSummary, CodexState, Settings } from "./types";
import * as api from "./api/codexSwitchApi";
import AppSidebar from "./components/AppSidebar.vue";
import AppTitlebar from "./components/AppTitlebar.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import { useAccounts } from "./composables/useAccounts";
import { useBackups } from "./composables/useBackups";
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

function selectTab(tab: Tab) {
  if (tab === "quota") {
    selectQuotaAccount();
    return;
  }
  selectedTab.value = tab;
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

      <AccountsView
        v-if="selectedTab === 'accounts'"
        v-model:query="query"
        :accounts="accounts"
        :filtered-accounts="filteredAccounts"
        :current="current"
        :busy="busy"
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
        @refresh-all="refreshAll"
        @fetch-usage="fetchUsage"
        @refresh-tokens="refreshTokens"
        @clear-usage="clearUsage"
      />

      <BackupsView
        v-else-if="selectedTab === 'backups'"
        :backups="backups"
        :busy="busy"
        @restore-backup="restoreBackup"
      />

      <SettingsView
        v-else
        :settings="settings"
        :busy="busy"
        :update-checking="updateChecking"
        :update-downloading="updateDownloading"
        :update-policy-source="updatePolicySource"
        :update-policy-error="updatePolicyError"
        @update-process-names="updateProcessNames"
        @check-for-updates="checkForUpdatesManually"
        @save-settings="saveSettings"
      />
    </section>

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
