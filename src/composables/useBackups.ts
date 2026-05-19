import type { Ref } from "vue";
import type { BackupSummary } from "../types";
import * as api from "../api/codexSwitchApi";
import type { ToastType } from "./useNotifications";

export function useBackups(deps: {
  backups: Ref<BackupSummary[]>;
  activeOperation: Ref<string>;
  refreshAll: () => Promise<void>;
  setMessage: (type: ToastType, message: string) => void;
}) {
  async function runOperation(key: string, work: () => Promise<void>) {
    deps.activeOperation.value = key;
    try {
      await work();
    } finally {
      if (deps.activeOperation.value === key) deps.activeOperation.value = "";
    }
  }

  async function createBackup() {
    await runOperation("backup:create", async () => {
      try {
        const backup = await api.backupCurrentState();
        await deps.refreshAll();
        deps.setMessage("success", `已创建备份 ${backup.id}。`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function restoreBackup(backup: BackupSummary) {
    const ok = window.confirm(`恢复备份 ${backup.id}？当前 auth.json/config.toml 会先被替换。`);
    if (!ok) return;
    await runOperation(`backup:restore:${backup.id}`, async () => {
      try {
        await api.restoreBackup(backup.id);
        await deps.refreshAll();
        deps.setMessage("success", `已恢复备份 ${backup.id}。`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  return {
    createBackup,
    restoreBackup
  };
}
