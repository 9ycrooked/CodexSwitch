import type { Ref } from "vue";
import { onMounted, onUnmounted } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AccountBundleImportResult,
  AccountSummary,
  CodexState,
  NetworkExitCheckResult,
  Settings
} from "../types";
import * as api from "../api/codexSwitchApi";
import type { ToastType } from "./useNotifications";

export function useAccounts(deps: {
  accounts: Ref<AccountSummary[]>;
  current: Ref<CodexState | null>;
  settings: Settings;
  activeOperation: Ref<string>;
  refreshAll: () => Promise<void>;
  setMessage: (type: ToastType, message: string) => void;
}) {
  function summarizeNetworkCheck(result: NetworkExitCheckResult) {
    const lines = [
      `整体状态：${result.overall_status}`,
      result.backend_country ? `后端出口国家：${result.backend_country}` : "",
      result.backend_ip ? `后端出口 IP：${result.backend_ip}` : "",
      result.auth_status != null ? `OAuth HTTP 状态：${result.auth_status}` : "",
      result.errors.length ? `错误：${result.errors.join("；")}` : "",
      result.warnings.length ? `警告：${result.warnings.join("；")}` : ""
    ].filter(Boolean);

    return lines.join("\n");
  }

  async function runOperation(key: string, work: () => Promise<void>) {
    deps.activeOperation.value = key;
    try {
      await work();
    } finally {
      if (deps.activeOperation.value === key) deps.activeOperation.value = "";
    }
  }

  function summarizeImportResult(results: AccountBundleImportResult[]) {
    const importedCount = results.reduce((sum, result) => sum + result.imported.length, 0);
    const failures = results.flatMap((result) => result.failed);
    if (!failures.length) {
      return { type: "success" as ToastType, message: `已导入 ${importedCount} 个账号和对应登录环境` };
    }

    const preview = failures
      .slice(0, 3)
      .map((failure) => failure.id || failure.path || failure.message)
      .join("；");
    return {
      type: importedCount ? ("warning" as ToastType) : ("error" as ToastType),
      message: `已导入 ${importedCount} 个账号，失败 ${failures.length} 个：${preview}`
    };
  }

  async function importBundlePaths(paths: string[]) {
    const bundlePaths = paths.filter((path) => path.toLowerCase().endsWith(".zip"));
    if (!bundlePaths.length) {
      deps.setMessage("error", "旧格式 .json / .toml 已不支持，请选择 Codex Switch 导出的 .zip 压缩包");
      return;
    }

    await runOperation("accounts:import", async () => {
      try {
        const results: AccountBundleImportResult[] = [];
        for (const path of bundlePaths) {
          results.push(await api.importAccountBundle(path));
        }
        await deps.refreshAll();
        const summary = summarizeImportResult(results);
        deps.setMessage(summary.type, summary.message);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function chooseAndImport() {
    const selected = await open({
      multiple: true,
      filters: [
        { name: "Codex Switch account bundle", extensions: ["zip"] },
        { name: "ZIP", extensions: ["zip"] }
      ]
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    if (!paths.length) return;
    await importBundlePaths(paths);
  }

  async function exportSelectedAccounts(accountIds: string[]) {
    if (!accountIds.length) {
      deps.setMessage("error", "请选择至少一个要导出的账号");
      return false;
    }

    const date = new Date().toISOString().slice(0, 10);
    const outputPath = await save({
      defaultPath: `codex-switch-accounts-${date}.zip`,
      filters: [{ name: "Codex Switch account bundle", extensions: ["zip"] }]
    });
    if (!outputPath) return false;

    let exported = false;
    await runOperation("accounts:export", async () => {
      try {
        const result = await api.exportAccountBundle(accountIds, outputPath);
        deps.setMessage(
          "success",
          `已导出 ${result.exported_count} 个账号，包含 ${result.included_profile_count} 个登录环境`
        );
        exported = true;
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
    return exported;
  }

  async function confirmNetworkForOAuth() {
    if (!deps.settings.check_oauth_network_on_login) return true;
    const networkResult = await api.checkOauthNetworkExit(deps.settings.check_egress_region);
    if (networkResult.overall_status === "ok") return true;
    const ok = window.confirm(
      `登录前网络检查未通过。\n\n${summarizeNetworkCheck(networkResult)}\n\n这不会阻止登录，但 token exchange 可能失败。仍要继续 OAuth 登录吗？`
    );
    if (!ok) {
      deps.setMessage("info", "OAuth 登录已取消");
      return false;
    }
    return true;
  }

  async function startOAuthLogin() {
    await runOperation("oauth:start", async () => {
      try {
        if (!(await confirmNetworkForOAuth())) return;

        const result = await api.startOauthLogin();
        const modeText = result.mode === "embedded" ? "内置 WebView2" : "外部隔离浏览器";
        deps.setMessage("info", `已打开 ${modeText} OAuth 登录 Profile: ${result.browser_profile_dir}`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function reloginAccount(account: AccountSummary) {
    const label = account.email || account.display_name || account.account_id || account.id;
    const ok = window.confirm(`将使用 ${label} 保存的登录环境重新 OAuth 登录，并保存登录得到的账号凭据。继续吗？`);
    if (!ok) return;

    await runOperation(`relogin:${account.id}`, async () => {
      try {
        if (!(await confirmNetworkForOAuth())) return;

        const result = await api.startAccountRelogin(account.id);
        const modeText = result.mode === "embedded" ? "内置 WebView2" : "外部隔离浏览器";
        deps.setMessage("info", `已用原登录环境打开 ${modeText}: ${result.browser_profile_dir}`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function closeOAuthLogin() {
    await runOperation("oauth:close", async () => {
      try {
        await api.closeOauthLogin();
        deps.setMessage("info", "已关闭等待中的 OAuth 登录窗口");
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
        deps.setMessage("success", `已刷新 ${updated.display_name} 的认证状态`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function deleteStoredAccount(account: AccountSummary, deleteProfile: boolean) {
    let deleted = false;
    await runOperation(`delete:${account.id}`, async () => {
      try {
        await api.deleteAccount(account.id, deleteProfile);
        await deps.refreshAll();
        deps.setMessage("success", `已删除 ${account.display_name}`);
        deleted = true;
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
    return deleted;
  }

  async function switchAccount(account: AccountSummary) {
    const label = account.email || account.display_name || account.account_id || account.id;
    const ok = window.confirm(
      `将切换到 ${label}。\n\n应用会快速关闭 Codex 桌面端，备份当前 auth.json/config.toml，写入目标账号，并在原本已打开 Codex 时重新打开。继续吗？`
    );
    if (!ok) return;

    await runOperation(`switch:${account.id}`, async () => {
      try {
        const result = await api.switchCodexAccount(account.id);
        await deps.refreshAll();
        const reopenText = result.codex_reopened ? "，已重新打开 Codex" : "";
        const warningText = result.warnings.length ? ` 警告：${result.warnings.join("；")}` : "";
        deps.setMessage(
          result.warnings.length ? "warning" : "success",
          `已切换到 ${result.account.display_name}，备份 ${result.backup_id} 已创建${reopenText}${warningText}`
        );
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  let unlistenDragDrop: (() => void) | null = null;
  onMounted(async () => {
    try {
      unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== "drop") return;
        const paths = event.payload.paths || [];
        if (paths.some((path) => /\.(zip|json|toml)$/i.test(path))) {
          void importBundlePaths(paths);
        }
      });
    } catch {
      unlistenDragDrop = null;
    }
  });

  onUnmounted(() => {
    if (unlistenDragDrop) unlistenDragDrop();
  });

  return {
    chooseAndImport,
    importBundlePaths,
    exportSelectedAccounts,
    startOAuthLogin,
    closeOAuthLogin,
    refreshTokens,
    reloginAccount,
    deleteStoredAccount,
    switchAccount
  };
}
