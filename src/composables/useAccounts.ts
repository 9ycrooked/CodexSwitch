import type { Ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type { AccountSummary, CodexState } from "../types";
import * as api from "../api/codexSwitchApi";
import type { ToastType } from "./useNotifications";

export function useAccounts(deps: {
  accounts: Ref<AccountSummary[]>;
  current: Ref<CodexState | null>;
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

    await runOperation("accounts:import", async () => {
      try {
        const imported = await api.importAccounts(paths);
        await deps.refreshAll();
        deps.setMessage("success", `已导入 ${imported.length} 个账号。`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function startOAuthLogin() {
    await runOperation("oauth:start", async () => {
      try {
        const result = await api.startOauthLogin();
        const modeText = result.mode === "embedded" ? "内置 WebView2" : "外部隔离浏览器";
        deps.setMessage("info", `已打开 ${modeText} OAuth 登录。Profile: ${result.browser_profile_dir}`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function closeOAuthLogin() {
    await runOperation("oauth:close", async () => {
      try {
        await api.closeOauthLogin();
        deps.setMessage("info", "已关闭等待中的 OAuth 登录窗口。");
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function refreshTokens(account: AccountSummary) {
    await runOperation(`refresh-token:${account.id}`, async () => {
      try {
        const updated = await api.refreshAccountTokens(account.id);
        await deps.refreshAll();
        deps.setMessage("success", `已刷新 ${updated.display_name} 的认证状态。`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function switchAccount(account: AccountSummary) {
    const label = account.email || account.display_name || account.account_id || account.id;
    const ok = window.confirm(
      `将切换到 ${label}。\n\n应用会先关闭 Codex 桌面端，备份当前 auth.json/config.toml，然后写入目标账号。继续吗？`
    );
    if (!ok) return;

    await runOperation(`switch:${account.id}`, async () => {
      try {
        const result = await api.switchCodexAccount(account.id);
        await deps.refreshAll();
        const warningText = result.warnings.length ? ` 警告：${result.warnings.join("；")}` : "";
        deps.setMessage(
          result.warnings.length ? "warning" : "success",
          `已切换到 ${result.account.display_name}，备份 ${result.backup_id} 已创建。${warningText}`
        );
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  return {
    chooseAndImport,
    startOAuthLogin,
    closeOAuthLogin,
    refreshTokens,
    switchAccount
  };
}
