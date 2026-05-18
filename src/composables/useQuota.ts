import { computed, ref, type Ref } from "vue";
import type { AccountSummary } from "../types";
import * as api from "../api/codexSwitchApi";
import { quotaLabel, usageLabel } from "../utils/format";

export function useQuota(deps: {
  accounts: Ref<AccountSummary[]>;
  busy: Ref<boolean>;
  refreshAll: () => Promise<void>;
  setMessage: (message: string, isError?: boolean) => void;
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

  async function checkQuota(account: AccountSummary) {
    const ok = window.confirm("额度探测会向 Codex 发送一个最小请求，可能产生极少量用量。继续吗？");
    if (!ok) return;
    deps.busy.value = true;
    try {
      const quota = await api.checkAccountQuota(account.id, account.plan?.includes("free") ? "gpt-5.5" : "gpt-5.5");
      await deps.refreshAll();
      deps.setMessage(`额度状态：${quotaLabel(quota)}。`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function fetchUsage(account?: AccountSummary | null) {
    const target = account || selectedQuotaAccount.value;
    if (!target) {
      deps.setMessage("请先选择一个账号。", true);
      return;
    }
    deps.busy.value = true;
    try {
      const state = await api.fetchCodexUsage(target.id);
      await deps.refreshAll();
      selectedQuotaAccountId.value = target.id;
      deps.setMessage(`额度状态：${usageLabel(state)}。`);
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  async function clearUsage(account?: AccountSummary | null) {
    const target = account || selectedQuotaAccount.value;
    if (!target) return;
    deps.busy.value = true;
    try {
      await api.clearUsageState(target.id);
      await deps.refreshAll();
      selectedQuotaAccountId.value = target.id;
      deps.setMessage("已清除该账号的额度记录。");
    } catch (err) {
      deps.setMessage(String(err), true);
    } finally {
      deps.busy.value = false;
    }
  }

  return {
    selectedQuotaAccountId,
    selectedQuotaAccount,
    selectedUsageState,
    selectQuotaAccount,
    checkQuota,
    fetchUsage,
    clearUsage
  };
}
