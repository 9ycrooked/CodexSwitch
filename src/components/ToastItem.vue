<script setup lang="ts">
import type { ToastMessage } from "../composables/useNotifications";

defineProps<{
  toast: ToastMessage;
  index: number;
}>();

defineEmits<{
  close: [string];
}>();

function ariaRole(type: ToastMessage["type"]) {
  return type === "error" ? "alert" : "status";
}

function ariaLive(type: ToastMessage["type"]) {
  return type === "error" ? "assertive" : "polite";
}
</script>

<template>
  <article
    :class="['toast-item', 'toast-' + toast.type]"
    :style="{ '--toast-index': index }"
    :role="ariaRole(toast.type)"
    :aria-live="ariaLive(toast.type)"
  >
    <span class="toast-stripe" aria-hidden="true"></span>
    <p class="toast-message">{{ toast.message }}</p>
    <button class="toast-close" type="button" aria-label="关闭通知" @click="$emit('close', toast.id)">×</button>
  </article>
</template>
