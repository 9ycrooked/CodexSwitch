import type { AccountSummary, CodexQuotaWindow, CodexState, QuotaState, UsageState } from "../types";

export function formatDate(value?: string | null) {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return date.toLocaleString();
}

export function quotaLabel(state?: QuotaState | null) {
  if (!state) return "未检查";
  const labels: Record<string, string> = {
    ok: "正常",
    cooldown: "冷却中",
    token_invalid: "认证失效",
    check_failed: "检查失败"
  };
  return labels[state.status] || state.status;
}

export function quotaClass(state?: QuotaState | null) {
  if (!state) return "muted";
  if (state.status === "ok") return "ok";
  if (state.status === "cooldown") return "warn";
  return "bad";
}

export function quotaTimestamp(state?: QuotaState | null) {
  if (!state) return "未检查";
  if (state.resets_at) return formatDate(state.resets_at);
  if (state.last_checked_at) return formatDate(state.last_checked_at);
  return "无时间记录";
}

export function usageLabel(state?: UsageState | null) {
  if (!state) return "未检查";
  const labels: Record<string, string> = {
    success: "已更新",
    cooldown: "冷却中",
    token_invalid: "认证失效",
    check_failed: "检查失败"
  };
  return labels[state.status] || state.status;
}

export function usageClass(state?: UsageState | null) {
  if (!state) return "muted";
  if (state.status === "success") return "ok";
  if (state.status === "cooldown") return "warn";
  return "bad";
}

export function usageWindowWidth(window: CodexQuotaWindow) {
  const value = Math.max(0, Math.min(100, Number(window.used_percent ?? 0)));
  return `${value}%`;
}

export function usageWindowClass(window: CodexQuotaWindow) {
  const value = Number(window.used_percent ?? 0);
  const isWeeklyWindow = window.id.includes("weekly") || window.label.includes("周");
  if (isWeeklyWindow && value < 100) return "quota-bar-fill quota-bar-fill-high";
  if (window.limit_reached || value >= 100) return "quota-bar-fill quota-bar-fill-low";
  if (value >= 90) return "quota-bar-fill quota-bar-fill-low";
  if (value >= 70) return "quota-bar-fill quota-bar-fill-medium";
  if (value > 0) return "quota-bar-fill quota-bar-fill-high";
  return "quota-bar-fill quota-bar-fill-muted";
}

export function usageWindowPercentClass(window: CodexQuotaWindow) {
  const value = Number(window.used_percent ?? 0);
  const isWeeklyWindow = window.id.includes("weekly") || window.label.includes("周");
  if (isWeeklyWindow && value < 100) return "ok";
  return window.limit_reached || value >= 100 ? "bad" : "ok";
}

export function usagePercentLabel(window: CodexQuotaWindow) {
  if (window.used_percent === null || window.used_percent === undefined) return "未知";
  return `已用 ${Math.round(Number(window.used_percent))}%`;
}

export function usageResetLabel(window: CodexQuotaWindow) {
  return window.reset_at ? formatDate(window.reset_at) : window.reset_label || "-";
}

export function isCurrentAccount(account: AccountSummary, current?: CodexState | null) {
  return Boolean(account.account_id && current?.current_account_id === account.account_id);
}

export function accountStatusLabel(account: AccountSummary, current?: CodexState | null) {
  if (isCurrentAccount(account, current)) return "当前";
  if (account.disabled) return "禁用";
  if (account.quota_state?.status === "cooldown") return "冷却";
  if (account.quota_state?.status === "token_invalid") return "失效";
  if (account.quota_state?.status === "check_failed") return "警告";
  return "可用";
}

export function accountStatusClass(account: AccountSummary, current?: CodexState | null) {
  if (isCurrentAccount(account, current)) return "state-badge-active";
  if (account.disabled) return "state-badge-disabled";
  if (account.quota_state?.status === "cooldown") return "state-badge-warning";
  if (account.quota_state?.status === "token_invalid" || account.quota_state?.status === "check_failed") {
    return "state-badge-disabled";
  }
  return "state-badge-active";
}

export function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
