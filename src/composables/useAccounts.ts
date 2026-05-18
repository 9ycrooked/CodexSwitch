import type { Ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type { AccountSummary, CodexState } from "../types";
import * as api from "../api/codexSwitchApi";

export function useAccounts(deps: {
  accounts: Ref<AccountSummary[]>;
  current: Ref<CodexState | null>;
  busy: Ref<boolean>;
  refreshAll: () => Promise<void>;
  setMessage: (message: string, isError?: boolean) => void;
}) {
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

    deps.busy.value = true;
    try {
      const imported = await api.importAccounts(paths);
      await deps.refreshAll();
      deps.setMessage(`已导入 ${imported.length} 个账号。`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function startOAuthLogin() {
    deps.busy.value = true;
    try {
      const result = await api.startOauthLogin();
      const modeText = result.mode === "embedded" ? "内置 WebView2" : "外部隔离浏览器";
      deps.setMessage(`已打开 ${modeText} OAuth 登录。Profile: ${result.browser_profile_dir}`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function closeOAuthLogin() {
    deps.busy.value = true;
    try {
      await api.closeOauthLogin();
      deps.setMessage("已关闭等待中的 OAuth 登录窗口。");
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function refreshTokens(account: AccountSummary) {
    deps.busy.value = true;
    try {
      const updated = await api.refreshAccountTokens(account.id);
      await deps.refreshAll();
      deps.setMessage(`已刷新 ${updated.display_name} 的认证状态。`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function switchAccount(account: AccountSummary) {
    const label = account.email || account.display_name || account.account_id || account.id;
    const ok = window.confirm(
      `将切换到 ${label}。\n\n应用会先关闭 Codex 桌面端，备份当前 auth.json/config.toml，然后写入目标账号。继续吗？`
    );
    if (!ok) return;

    deps.busy.value = true;
    try {
      const result = await api.switchCodexAccount(account.id);
      await deps.refreshAll();
      const warningText = result.warnings.length ? ` 警告：${result.warnings.join("；")}` : "";
      deps.setMessage(`已切换到 ${result.account.display_name}，备份 ${result.backup_id} 已创建。${warningText}`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  return {
    chooseAndImport,
    startOAuthLogin,
    closeOAuthLogin,
    refreshTokens,
    switchAccount
  };
}
