<script setup lang="ts">
import { Minus, Square, X } from "lucide-vue-next";

defineProps<{
  onMinimize: () => void;
  onToggleMaximize: () => void;
  onClose: () => void;
  onStartDrag: (event: MouseEvent) => void;
  onDoubleClick: (event: MouseEvent) => void;
}>();
</script>

<template>
  <header class="app-titlebar">
    <div class="titlebar-brand" @mousedown="onStartDrag" @dblclick="onDoubleClick">
      <span class="titlebar-mark" aria-hidden="true"></span>
      <span>Codex Switch</span>
    </div>
    <div class="titlebar-drag-region" @mousedown="onStartDrag" @dblclick="onDoubleClick"></div>
    <div class="titlebar-window-controls">
      <button class="titlebar-button" type="button" aria-label="最小化" @click="onMinimize">
        <Minus :size="14" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button class="titlebar-button" type="button" aria-label="最大化或还原" @click="onToggleMaximize">
        <Square :size="13" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button class="titlebar-button titlebar-close" type="button" aria-label="关闭" @click="onClose">
        <X :size="15" :stroke-width="1.8" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>
