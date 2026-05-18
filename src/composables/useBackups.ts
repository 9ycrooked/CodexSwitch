import type { Ref } from "vue";
import type { BackupSummary } from "../types";
import * as api from "../api/codexSwitchApi";

export function useBackups(deps: {
  backups: Ref<BackupSummary[]>;
  busy: Ref<boolean>;
  refreshAll: () => Promise<void>;
  setMessage: (message: string, isError?: boolean) => void;
}) {
  async function createBackup() {
    deps.busy.value = true;
    try {
      const backup = await api.backupCurrentState();
      await deps.refreshAll();
      deps.setMessage(`已创建备份 ${backup.id}。`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function restoreBackup(backup: BackupSummary) {
    const ok = window.confirm(`恢复备份 ${backup.id}？当前 auth.json/config.toml 会先被替换。`);
    if (!ok) return;
    deps.busy.value = true;
    try {
      await api.restoreBackup(backup.id);
      await deps.refreshAll();
      deps.setMessage(`已恢复备份 ${backup.id}。`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  return {
    createBackup,
    restoreBackup
  };
}
