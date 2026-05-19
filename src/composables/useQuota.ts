import { computed, ref, type Ref } from "vue";
import type { AccountSummary } from "../types";
import * as api from "../api/codexSwitchApi";
import { quotaLabel, usageLabel } from "../utils/format";
import type { ToastType } from "./useNotifications";

export function useQuota(deps: {
  accounts: Ref<AccountSummary[]>;
  activeOperation: Ref<string>;
  refreshAll: () => Promise<void>;
  setMessage: (type: ToastType, message: string) => void;
}) {
  const selectedQuotaAccountId = ref("");
  const selectedQuotaAccount = computed(() => {
    return deps.accounts.value.find((account) => account.id === selectedQuotaAccountId.value) || deps.accounts.value[0] || null;
  });
  const selectedUsageState = computed(() => selectedQuotaAccount.value?.usage_state || null);

  function selectQuotaAccount(account?: AccountSummary | null) {
    if (account) selectedQuotaAccountId.value = account.id;
    if (!selectedQuotaAccountId.value && deps.accounts.value[0]) selectedQuotaAccountId.value = deps.accounts.value[0].id;
  }

  async function runOperation(key: string, work: () => Promise<void>) {
    deps.activeOperation.value = key;
    try {
      await work();
    } finally {
      if (deps.activeOperation.value === key) deps.activeOperation.value = "";
    }
  }

  async function checkQuota(account: AccountSummary) {
    const ok = window.confirm("额度探测会向 Codex 发送一个最小请求，可能产生极少量用量。继续吗？");
    if (!ok) return;
    await runOperation(`quota:${account.id}`, async () => {
      try {
        const quota = await api.checkAccountQuota(account.id, account.plan?.includes("free") ? "gpt-5.5" : "gpt-5.5");
        await deps.refreshAll();
        deps.setMessage("success", `额度状态：${quotaLabel(quota)}。`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function fetchUsage(account?: AccountSummary | null) {
    const target = account || selectedQuotaAccount.value;
    if (!target) {
      deps.setMessage("warning", "请先选择一个账号。");
      return;
    }
    await runOperation(`quota:${target.id}`, async () => {
      try {
        const state = await api.fetchCodexUsage(target.id);
        await deps.refreshAll();
        selectedQuotaAccountId.value = target.id;
        deps.setMessage("success", `额度状态：${usageLabel(state)}。`);
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  async function fetchAllUsage() {
    const targets = deps.accounts.value.filter((account) => !account.disabled);
    if (!targets.length) {
      deps.setMessage("warning", "没有可检查的账号。");
      return;
    }

    await runOperation("quota:all", async () => {
      let succeeded = 0;
      let failed = 0;

      for (const account of targets) {
        try {
          await api.fetchCodexUsage(account.id);
          succeeded += 1;
        } catch {
          failed += 1;
        }
      }

      await deps.refreshAll();
      deps.setMessage(failed > 0 ? "warning" : "success", `全部额度检查完成：成功 ${succeeded} 个，失败 ${failed} 个。`);
    });
  }

  async function clearUsage(account?: AccountSummary | null) {
    const target = account || selectedQuotaAccount.value;
    if (!target) return;
    await runOperation(`quota:${target.id}`, async () => {
      try {
        await api.clearUsageState(target.id);
        await deps.refreshAll();
        selectedQuotaAccountId.value = target.id;
        deps.setMessage("success", "已清除该账号的额度记录。");
      } catch (err) {
        deps.setMessage("error", String(err));
      }
    });
  }

  return {
    selectedQuotaAccountId,
    selectedQuotaAccount,
    selectedUsageState,
    selectQuotaAccount,
    checkQuota,
    fetchUsage,
    fetchAllUsage,
    clearUsage
  };
}
