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
</script>

<template>
  <article
    :class="['toast-item', 'toast-' + toast.type]"
    :style="{ '--toast-index': index }"
    :role="ariaRole(toast.type)"
    aria-live="polite"
  >
    <span class="toast-stripe" aria-hidden="true"></span>
    <p class="toast-message">{{ toast.message }}</p>
    <button class="toast-close" type="button" aria-label="关闭通知" @click="$emit('close', toast.id)">×</button>
  </article>
</template>
