import { computed, markRaw, reactive, ref, shallowRef } from "vue";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import type { Settings, UpdatePolicy } from "../types";
import { formatError } from "../utils/format";
import type { ToastType } from "./useNotifications";

const UPDATE_POLICY_URL = "https://github.com/9ycrooked/CodexSwitch/releases/latest/download/update-policy.json";

type UpdateInfo = Update & {
  body?: string;
  notes?: string;
  version?: string;
  currentVersion?: string;
};

export function useUpdater(settings: Settings, setMessage: (type: ToastType, message: string) => void) {
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
  const lastUpdateCheckedAt = ref<string | null>(null);
  const pendingUpdate = shallowRef<Update | null>(null);
  const updateDownloadedBytes = ref(0);
  const updateTotalBytes = ref(0);

  const pendingUpdateInfo = computed(() => pendingUpdate.value as UpdateInfo | null);
  const pendingUpdateNotes = computed(() => {
    return pendingUpdateInfo.value?.body || pendingUpdateInfo.value?.notes || "这个版本没有填写更新说明。";
  });
  const updateProgressPercent = computed(() => {
    if (!updateTotalBytes.value) return 0;
    return Math.min(100, Math.round((updateDownloadedBytes.value / updateTotalBytes.value) * 100));
  });
  const updateIsForced = computed(() => Boolean(updatePolicy.force_update_on_startup && pendingUpdate.value));

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
    if (!manual && settings.manual_update_check_only) {
      Object.assign(updatePolicy, {
        check_updates_on_startup: false,
        force_update_on_startup: false,
        message: null
      });
      updatePolicySource.value = "仅手动检查";
      updatePolicyError.value = "";
      return;
    }

    const policy = await loadUpdatePolicy();
    if (!manual && !policy.check_updates_on_startup) return;

    updateChecking.value = true;
    updateError.value = "";

    try {
      const update = await check();
      if (!update) {
        if (manual) setMessage("info", "当前已经是最新版本");
        return;
      }

      pendingUpdate.value = markRaw(update);
      updateDialogOpen.value = true;
    } catch (err) {
      const message = `更新检查失败：${formatError(err)}`;
      updateError.value = message;
      if (manual) setMessage("error", message);
    } finally {
      updateChecking.value = false;
      lastUpdateCheckedAt.value = new Date().toISOString();
    }
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

  return {
    updatePolicy,
    updatePolicySource,
    updatePolicyError,
    updateDialogOpen,
    updateChecking,
    updateDownloading,
    updateError,
    lastUpdateCheckedAt,
    pendingUpdate,
    pendingUpdateInfo,
    pendingUpdateNotes,
    updateDownloadedBytes,
    updateTotalBytes,
    updateProgressPercent,
    updateIsForced,
    runUpdateCheck,
    checkForUpdatesManually: () => runUpdateCheck({ manual: true }),
    installPendingUpdate,
    dismissUpdateDialog
  };
}
