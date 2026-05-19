<script setup lang="ts">
defineProps<{
  open: boolean;
  forced: boolean;
  downloading: boolean;
  error: string;
  policyMessage?: string | null;
  currentVersion?: string;
  nextVersion?: string;
  notes: string;
  progressPercent: number;
  hasTotalBytes: boolean;
}>();

defineEmits<{
  dismiss: [];
  install: [];
  closeApp: [];
}>();
</script>

<template>
  <div v-if="open" class="modal-backdrop">
    <section class="modal update-modal" role="dialog" aria-modal="true" aria-labelledby="update-modal-title">
      <div class="modal-header">
        <div>
          <p class="eyebrow">软件更新</p>
          <h2 id="update-modal-title">发现新版本</h2>
        </div>
        <button
          v-if="!forced"
          class="modal-close"
          type="button"
          aria-label="关闭更新提示"
          :disabled="downloading"
          @click="$emit('dismiss')"
        >
          ×
        </button>
      </div>

      <p v-if="policyMessage" class="update-policy-message">{{ policyMessage }}</p>

      <div class="update-version-row">
        <span>当前版本 {{ currentVersion || "未知" }}</span>
        <span>新版本 {{ nextVersion || "未知" }}</span>
      </div>

      <pre class="update-notes">{{ notes }}</pre>

      <div v-if="downloading" class="update-progress">
        <div class="quota-bar" aria-hidden="true">
          <div
            class="quota-bar-fill quota-bar-fill-high"
            :style="{ width: hasTotalBytes ? `${progressPercent}%` : '35%' }"
          ></div>
        </div>
        <p>{{ hasTotalBytes ? `${progressPercent}%` : "正在下载更新..." }}</p>
      </div>

      <p v-if="error" class="update-error-message">{{ error }}</p>

      <div class="modal-actions">
        <button
          v-if="!forced"
          class="secondary"
          type="button"
          :disabled="downloading"
          @click="$emit('dismiss')"
        >
          稍后
        </button>
        <button v-else class="secondary" type="button" :disabled="downloading" @click="$emit('closeApp')">
          退出
        </button>
        <button type="button" :disabled="downloading" @click="$emit('install')">
          {{ downloading ? "正在更新" : "立即更新" }}
        </button>
      </div>
    </section>
  </div>
</template>
