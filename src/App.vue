<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { Minus, Square, X } from "lucide-vue-next";

type AccountSummary = {
  id: string;
  display_name: string;
  email?: string | null;
  account_id?: string | null;
  plan?: string | null;
  expired?: string | null;
  disabled: boolean;
  imported_at: string;
  has_config: boolean;
  browser_profile_dir?: string | null;
  oauth_metadata?: OAuthMetadata | null;
  quota_state?: QuotaState | null;
  usage_state?: UsageState | null;
};

type OAuthMetadata = {
  email?: string | null;
  account_id?: string | null;
  plan_type?: string | null;
  subscription_until?: string | null;
};

type QuotaState = {
  status: string;
  last_checked_at?: string | null;
  last_error?: string | null;
  resets_at?: string | null;
  resets_in_seconds?: number | null;
  model?: string | null;
};

type UsageState = {
  status: string;
  last_checked_at?: string | null;
  last_error?: string | null;
  http_status?: number | null;
  resets_at?: string | null;
  raw_plan_type?: string | null;
  windows: CodexQuotaWindow[];
};

type CodexQuotaWindow = {
  id: string;
  label: string;
  used_percent?: number | null;
  reset_at?: string | null;
  reset_label: string;
  limit_reached: boolean;
};

type BackupSummary = {
  id: string;
  created_at: string;
  auth_path?: string | null;
  config_path?: string | null;
};

type Settings = {
  codex_home: string;
  process_names: string[];
  close_timeout_ms: number;
  browser_profile_dir: string;
  oauth_callback_port: number;
  keep_login_profiles: boolean;
  oauth_login_mode: string;
  check_updates_on_startup: boolean;
  force_update_on_startup: boolean;
};

type UpdatePolicy = {
  check_updates_on_startup: boolean;
  force_update_on_startup: boolean;
  message?: string | null;
};

type UpdateInfo = Update & {
  body?: string;
  notes?: string;
  version?: string;
  currentVersion?: string;
};

type CodexState = {
  codex_home: string;
  auth_exists: boolean;
  config_exists: boolean;
  current_account_id?: string | null;
  current_email?: string | null;
  current_auth_mode?: string | null;
};

type SwitchResult = {
  account: AccountSummary;
  backup_id: string;
  warnings: string[];
};

const UPDATE_POLICY_URL = "https://github.com/9ycrooked/CodexSwitch/releases/latest/download/update-policy.json";

const accounts = ref<AccountSummary[]>([]);
const backups = ref<BackupSummary[]>([]);
const current = ref<CodexState | null>(null);
const selectedTab = ref<"accounts" | "quota" | "backups" | "settings">("accounts");
const query = ref("");
const busy = ref(false);
const notice = ref("");
const error = ref("");
const selectedQuotaAccountId = ref("");
const appWindow = getCurrentWindow();

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

const updatePolicy = reactive<UpdatePolicy>({
  check_updates_on_startup: true,
  force_update_on_startup: false,
  message: null
});
const updatePolicySource = ref("默认策略");
const updatePolicyError = ref("");
const updateDialogOpen = ref(false);
const updateChecking = ref(false);
const updateDownloading = ref(false);
const updateError = ref("");
const pendingUpdate = ref<Update | null>(null);
const updateDownloadedBytes = ref(0);
const updateTotalBytes = ref(0);

const filteredAccounts = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return accounts.value;
  return accounts.value.filter((account) => {
    return [account.display_name, account.email, account.account_id, account.plan]
      .filter(Boolean)
      .some((value) => String(value).toLowerCase().includes(needle));
  });
});

const selectedQuotaAccount = computed(() => {
  return accounts.value.find((account) => account.id === selectedQuotaAccountId.value) || accounts.value[0] || null;
});

const selectedUsageState = computed(() => selectedQuotaAccount.value?.usage_state || null);

const pendingUpdateInfo = computed(() => pendingUpdate.value as UpdateInfo | null);

const pendingUpdateNotes = computed(() => {
  return pendingUpdateInfo.value?.body || pendingUpdateInfo.value?.notes || "这个版本没有填写更新说明。";
});

const updateProgressPercent = computed(() => {
  if (!updateTotalBytes.value) return 0;
  return Math.min(100, Math.round((updateDownloadedBytes.value / updateTotalBytes.value) * 100));
});

const updateIsForced = computed(() => Boolean(updatePolicy.force_update_on_startup && pendingUpdate.value));

function formatDate(value?: string | null) {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return date.toLocaleString();
}

function isCurrentAccount(account: AccountSummary) {
  return Boolean(account.account_id && current.value?.current_account_id === account.account_id);
}

function updateProcessNames(event: Event) {
  settings.process_names = (event.target as HTMLInputElement).value.split(",");
}

function quotaLabel(state?: QuotaState | null) {
  if (!state) return "未检查";
  const labels: Record<string, string> = {
    ok: "正常",
    cooldown: "冷却中",
    token_invalid: "认证失效",
    check_failed: "检查失败"
  };
  return labels[state.status] || state.status;
}

function quotaClass(state?: QuotaState | null) {
  if (!state) return "muted";
  if (state.status === "ok") return "ok";
  if (state.status === "cooldown") return "warn";
  return "bad";
}

function quotaMeterClass(state?: QuotaState | null) {
  const statusClass = quotaClass(state);
  if (statusClass === "ok") return "quota-bar-fill quota-bar-fill-high";
  if (statusClass === "warn") return "quota-bar-fill quota-bar-fill-medium";
  if (statusClass === "bad") return "quota-bar-fill quota-bar-fill-low";
  return "quota-bar-fill quota-bar-fill-muted";
}

function quotaMeterWidth(state?: QuotaState | null) {
  if (!state) return "0%";
  const widths: Record<string, string> = {
    ok: "100%",
    cooldown: "58%",
    token_invalid: "28%",
    check_failed: "36%"
  };
  return widths[state.status] || "24%";
}

function quotaTimestamp(state?: QuotaState | null) {
  if (!state) return "未检查";
  if (state.resets_at) return formatDate(state.resets_at);
  if (state.last_checked_at) return formatDate(state.last_checked_at);
  return "无时间记录";
}

function usageLabel(state?: UsageState | null) {
  if (!state) return "未检查";
  const labels: Record<string, string> = {
    success: "已更新",
    cooldown: "冷却中",
    token_invalid: "认证失效",
    check_failed: "检查失败"
  };
  return labels[state.status] || state.status;
}

function usageClass(state?: UsageState | null) {
  if (!state) return "muted";
  if (state.status === "success") return "ok";
  if (state.status === "cooldown") return "warn";
  return "bad";
}

function usageWindowWidth(window: CodexQuotaWindow) {
  const value = Math.max(0, Math.min(100, Number(window.used_percent ?? 0)));
  return `${value}%`;
}

function usageWindowClass(window: CodexQuotaWindow) {
  const value = Number(window.used_percent ?? 0);
  const isWeeklyWindow = window.id.includes("weekly") || window.label.includes("周");
  if (isWeeklyWindow && value < 100) return "quota-bar-fill quota-bar-fill-high";
  if (window.limit_reached || value >= 100) return "quota-bar-fill quota-bar-fill-low";
  if (value >= 90) return "quota-bar-fill quota-bar-fill-low";
  if (value >= 70) return "quota-bar-fill quota-bar-fill-medium";
  if (value > 0) return "quota-bar-fill quota-bar-fill-high";
  return "quota-bar-fill quota-bar-fill-muted";
}

function usageWindowPercentClass(window: CodexQuotaWindow) {
  const value = Number(window.used_percent ?? 0);
  const isWeeklyWindow = window.id.includes("weekly") || window.label.includes("周");
  if (isWeeklyWindow && value < 100) return "ok";
  return window.limit_reached || value >= 100 ? "bad" : "ok";
}

function usagePercentLabel(window: CodexQuotaWindow) {
  if (window.used_percent === null || window.used_percent === undefined) return "未知";
  return `已用 ${Math.round(Number(window.used_percent))}%`;
}

function usageResetLabel(window: CodexQuotaWindow) {
  return window.reset_at ? formatDate(window.reset_at) : window.reset_label || "-";
}

function accountStatusLabel(account: AccountSummary) {
  if (isCurrentAccount(account)) return "当前";
  if (account.disabled) return "禁用";
  if (account.quota_state?.status === "cooldown") return "冷却";
  if (account.quota_state?.status === "token_invalid") return "失效";
  if (account.quota_state?.status === "check_failed") return "警告";
  return "可用";
}

function accountStatusClass(account: AccountSummary) {
  if (isCurrentAccount(account)) return "state-badge-active";
  if (account.disabled) return "state-badge-disabled";
  if (account.quota_state?.status === "cooldown") return "state-badge-warning";
  if (account.quota_state?.status === "token_invalid" || account.quota_state?.status === "check_failed") {
    return "state-badge-disabled";
  }
  return "state-badge-active";
}

function setMessage(message: string, isError = false) {
  notice.value = isError ? "" : message;
  error.value = isError ? message : "";
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function toBoolean(value: unknown, fallback: boolean) {
  return typeof value === "boolean" ? value : fallback;
}

async function loadUpdatePolicy(): Promise<UpdatePolicy> {
  const fallback: UpdatePolicy = {
    check_updates_on_startup: settings.check_updates_on_startup ?? true,
    force_update_on_startup: settings.force_update_on_startup ?? false,
    message: null
  };

  try {
    const response = await fetch(UPDATE_POLICY_URL, { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const remote = (await response.json()) as Partial<UpdatePolicy>;
    const nextPolicy = {
      check_updates_on_startup: toBoolean(remote.check_updates_on_startup, fallback.check_updates_on_startup),
      force_update_on_startup: toBoolean(remote.force_update_on_startup, fallback.force_update_on_startup),
      message: typeof remote.message === "string" ? remote.message : null
    };

    Object.assign(updatePolicy, nextPolicy);
    updatePolicySource.value = "远程发布配置";
    updatePolicyError.value = "";
    return nextPolicy;
  } catch (err) {
    Object.assign(updatePolicy, fallback);
    updatePolicySource.value = "默认策略";
    updatePolicyError.value = `发布配置读取失败，已使用默认策略：${formatError(err)}`;
    return fallback;
  }
}

async function runUpdateCheck(options: { manual?: boolean } = {}) {
  const manual = Boolean(options.manual);
  const policy = await loadUpdatePolicy();
  if (!manual && !policy.check_updates_on_startup) return;

  updateChecking.value = true;
  updateError.value = "";

  try {
    const update = await check();
    if (!update) {
      if (manual) setMessage("当前已经是最新版本。");
      return;
    }

    pendingUpdate.value = update;
    updateDialogOpen.value = true;
    if (manual) setMessage("");
  } catch (err) {
    const message = `更新检查失败：${formatError(err)}`;
    updateError.value = message;
    if (manual) setMessage(message, true);
  } finally {
    updateChecking.value = false;
  }
}

async function checkForUpdatesManually() {
  await runUpdateCheck({ manual: true });
}

async function installPendingUpdate() {
  if (!pendingUpdate.value) return;

  updateDownloading.value = true;
  updateError.value = "";
  updateDownloadedBytes.value = 0;
  updateTotalBytes.value = 0;

  try {
    await pendingUpdate.value.downloadAndInstall((event) => {
      if (event.event === "Started") {
        updateTotalBytes.value = event.data.contentLength ?? 0;
      }
      if (event.event === "Progress") {
        updateDownloadedBytes.value += event.data.chunkLength;
      }
    });

    await relaunch();
  } catch (err) {
    updateError.value = `更新安装失败：${formatError(err)}`;
  } finally {
    updateDownloading.value = false;
  }
}

function dismissUpdateDialog() {
  if (updateIsForced.value) return;
  updateDialogOpen.value = false;
}

async function minimizeWindow() {
  await appWindow.minimize();
}

async function toggleMaximizeWindow() {
  await appWindow.toggleMaximize();
}

async function closeWindow() {
  await appWindow.close();
}

async function startWindowDrag(event: MouseEvent) {
  if (event.button !== 0) return;
  await appWindow.startDragging();
}

async function handleTitlebarDoubleClick(event: MouseEvent) {
  if (event.button !== 0) return;
  await appWindow.toggleMaximize();
}

async function refreshAll() {
  const [nextAccounts, nextBackups, nextCurrent, nextSettings] = await Promise.all([
    invoke<AccountSummary[]>("list_accounts"),
    invoke<BackupSummary[]>("list_backups"),
    invoke<CodexState>("read_current_codex_state"),
    invoke<Settings>("read_settings")
  ]);
  accounts.value = nextAccounts;
  backups.value = nextBackups;
  current.value = nextCurrent;
  Object.assign(settings, nextSettings);
}

async function chooseAndImport() {
  const selected = await open({
    multiple: true,
    filters: [
      { name: "Codex account files", extensions: ["json", "toml"] },
      { name: "JSON", extensions: ["json"] },
      { name: "TOML", extensions: ["toml"] }
    ]
  });
  const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
  if (!paths.length) return;

  busy.value = true;
  try {
    const imported = await invoke<AccountSummary[]>("import_accounts", { paths });
    await refreshAll();
    setMessage(`已导入 ${imported.length} 个账号。`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function startOAuthLogin() {
  busy.value = true;
  try {
    const result = await invoke<{ auth_url: string; browser_profile_dir: string; mode: string }>("start_oauth_login", {
      profileId: null
    });
    const modeText = result.mode === "embedded" ? "内置 WebView2" : "外部隔离浏览器";
    setMessage(`已打开 ${modeText} OAuth 登录。Profile: ${result.browser_profile_dir}`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function closeOAuthLogin() {
  busy.value = true;
  try {
    await invoke("close_oauth_login");
    setMessage("已关闭等待中的 OAuth 登录窗口。");
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function refreshTokens(account: AccountSummary) {
  busy.value = true;
  try {
    const updated = await invoke<AccountSummary>("refresh_account_tokens", { accountId: account.id });
    await refreshAll();
    setMessage(`已刷新 ${updated.display_name} 的认证状态。`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function checkQuota(account: AccountSummary) {
  const ok = window.confirm("额度探测会向 Codex 发送一个最小请求，可能产生极少量用量。继续吗？");
  if (!ok) return;
  busy.value = true;
  try {
    const quota = await invoke<QuotaState>("check_account_quota", {
      accountId: account.id,
      model: account.plan?.includes("free") ? "gpt-5.5" : "gpt-5.5"
    });
    await refreshAll();
    setMessage(`额度状态：${quotaLabel(quota)}。`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

function selectQuotaAccount(account?: AccountSummary | null) {
  if (account) selectedQuotaAccountId.value = account.id;
  if (!selectedQuotaAccountId.value && accounts.value[0]) selectedQuotaAccountId.value = accounts.value[0].id;
  selectedTab.value = "quota";
}

async function fetchUsage(account?: AccountSummary | null) {
  const target = account || selectedQuotaAccount.value;
  if (!target) {
    setMessage("请先选择一个账号。", true);
    return;
  }
  busy.value = true;
  try {
    const state = await invoke<UsageState>("fetch_codex_usage", { accountId: target.id });
    await refreshAll();
    selectedQuotaAccountId.value = target.id;
    setMessage(`额度状态：${usageLabel(state)}。`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function clearUsage(account?: AccountSummary | null) {
  const target = account || selectedQuotaAccount.value;
  if (!target) return;
  busy.value = true;
  try {
    await invoke("clear_usage_state", { accountId: target.id });
    await refreshAll();
    selectedQuotaAccountId.value = target.id;
    setMessage("已清除该账号的额度记录。");
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function switchAccount(account: AccountSummary) {
  const label = account.email || account.display_name || account.account_id || account.id;
  const ok = window.confirm(
    `将切换到 ${label}。\n\n应用会先关闭 Codex 桌面端，备份当前 auth.json/config.toml，然后写入目标账号。继续吗？`
  );
  if (!ok) return;

  busy.value = true;
  try {
    const result = await invoke<SwitchResult>("switch_account", { accountId: account.id });
    await refreshAll();
    const warningText = result.warnings.length ? ` 警告：${result.warnings.join("；")}` : "";
    setMessage(`已切换到 ${result.account.display_name}，备份 ${result.backup_id} 已创建。${warningText}`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function createBackup() {
  busy.value = true;
  try {
    const backup = await invoke<BackupSummary>("backup_current_state");
    await refreshAll();
    setMessage(`已创建备份 ${backup.id}。`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function restoreBackup(backup: BackupSummary) {
  const ok = window.confirm(`恢复备份 ${backup.id}？当前 auth.json/config.toml 会先被替换。`);
  if (!ok) return;
  busy.value = true;
  try {
    await invoke("restore_backup", { backupId: backup.id });
    await refreshAll();
    setMessage(`已恢复备份 ${backup.id}。`);
  } catch (err) {
    setMessage(String(err), true);
  } finally {
    busy.value = false;
  }
}

async function saveSettings() {
  busy.value = true;
  try {
    const processNames = settings.process_names
      .flatMap((item) => item.split(","))
      .map((item) => item.trim())
      .filter(Boolean);
    const saved = await invoke<Settings>("update_settings", {
      settings: {
        codex_home: settings.codex_home,
        process_names: processNames,
        close_timeout_ms: Number(settings.close_timeout_ms),
        browser_profile_dir: settings.browser_profile_dir,
        oauth_callback_port: Number(settings.oauth_callback_port),
        keep_login_profiles: Boolean(settings.keep_login_profiles),
        oauth_login_mode: settings.oauth_login_mode,
        check_updates_on_startup: settings.check_updates_on_startup,
        force_update_on_startup: settings.force_update_on_startup
      }
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
            :class="['account-card file-card', { current: isCurrentAccount(account), disabled: account.disabled }]"
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
                      <span :class="['state-badge', accountStatusClass(account)]">
                        {{ accountStatusLabel(account) }}
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
                  <button class="primary-action-button" :disabled="busy || isCurrentAccount(account)" @click="switchAccount(account)">
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
                  <span :class="['state-badge', accountStatusClass(account)]">
                    {{ accountStatusLabel(account) }}
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
