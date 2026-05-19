<script setup lang="ts">
import type { ToastMessage } from "../composables/useNotifications";
import ToastItem from "./ToastItem.vue";

defineProps<{
  toasts: ToastMessage[];
}>();

defineEmits<{
  close: [string];
  pause: [];
  resume: [];
}>();
</script>

<template>
  <TransitionGroup
    v-if="toasts.length"
    tag="section"
    class="toast-viewport"
    name="toast-stack"
    aria-label="应用通知"
    @mouseenter="$emit('pause')"
    @mouseleave="$emit('resume')"
  >
    <ToastItem
      v-for="(toast, index) in toasts"
      :key="toast.id"
      :toast="toast"
      :index="index"
      @close="$emit('close', $event)"
    />
  </TransitionGroup>
</template>
