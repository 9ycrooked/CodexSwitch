import { computed, ref } from "vue";

export type ToastType = "success" | "info" | "warning" | "error";

export type ToastMessage = {
  id: string;
  type: ToastType;
  message: string;
  createdAt: number;
  durationMs: number;
  remainingMs: number;
  timerStartedAt: number | null;
};

const DEFAULT_DURATIONS: Record<ToastType, number> = {
  success: 3000,
  info: 3000,
  warning: 5000,
  error: 8000
};

const MAX_TOASTS = 4;

let nextToastId = 1;

function createId() {
  const id = `toast-${Date.now()}-${nextToastId}`;
  nextToastId += 1;
  return id;
}

export function useNotifications() {
  const toasts = ref<ToastMessage[]>([]);
  const timers = new Map<string, number>();
  const isPaused = ref(false);

  const visibleToasts = computed(() => toasts.value.slice(0, MAX_TOASTS));

  function clearTimer(id: string) {
    const timer = timers.get(id);
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timers.delete(id);
    }
  }

  function remove(id: string) {
    clearTimer(id);
    toasts.value = toasts.value.filter((toast) => toast.id !== id);
  }

  function startTimer(toast: ToastMessage) {
    clearTimer(toast.id);
    if (isPaused.value || toast.remainingMs <= 0) return;

    toast.timerStartedAt = Date.now();
    const timer = window.setTimeout(() => remove(toast.id), toast.remainingMs);
    timers.set(toast.id, timer);
  }

  function trimQueue() {
    const removed = toasts.value.slice(MAX_TOASTS);
    for (const toast of removed) clearTimer(toast.id);
    toasts.value = toasts.value.slice(0, MAX_TOASTS);
  }

  function add(type: ToastType, message: string, durationMs = DEFAULT_DURATIONS[type]) {
    const trimmedMessage = message.trim();
    if (!trimmedMessage) return null;

    const toast: ToastMessage = {
      id: createId(),
      type,
      message: trimmedMessage,
      createdAt: Date.now(),
      durationMs,
      remainingMs: durationMs,
      timerStartedAt: null
    };

    toasts.value = [toast, ...toasts.value];
    trimQueue();
    startTimer(toast);
    return toast.id;
  }

  function pause() {
    if (isPaused.value) return;
    isPaused.value = true;
    const now = Date.now();

    for (const toast of toasts.value) {
      clearTimer(toast.id);
      if (toast.timerStartedAt !== null) {
        toast.remainingMs = Math.max(0, toast.remainingMs - (now - toast.timerStartedAt));
        toast.timerStartedAt = null;
      }
    }
  }

  function resume() {
    if (!isPaused.value) return;
    isPaused.value = false;
    for (const toast of toasts.value) startTimer(toast);
  }

  function clear() {
    for (const toast of toasts.value) clearTimer(toast.id);
    toasts.value = [];
  }

  return {
    toasts: visibleToasts,
    add,
    remove,
    pause,
    resume,
    clear,
    success: (message: string) => add("success", message),
    info: (message: string) => add("info", message),
    warning: (message: string) => add("warning", message),
    error: (message: string) => add("error", message)
  };
}
