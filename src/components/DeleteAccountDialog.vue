<script setup lang="ts">
import type { AccountSummary } from "../types";

defineProps<{
  open: boolean;
  account: AccountSummary | null;
  deleteProfile: boolean;
  deleting: boolean;
}>();

defineEmits<{
  "update:deleteProfile": [boolean];
  cancel: [];
  confirm: [];
}>();

function checkedValue(event: Event) {
  return (event.target as HTMLInputElement).checked;
}
</script>

<template>
  <div v-if="open && account" class="modal-backdrop">
    <section class="modal delete-account-modal" role="dialog" aria-modal="true" aria-labelledby="delete-account-title">
      <div class="modal-header">
        <div>
          <p class="eyebrow">账号库</p>
          <h2 id="delete-account-title">删除账号</h2>
        </div>
        <button class="modal-close" type="button" aria-label="关闭删除确认" :disabled="deleting" @click="$emit('cancel')">
          ×
        </button>
      </div>

      <div class="delete-account-summary">
        <strong>{{ account.display_name }}</strong>
        <span>{{ account.email || account.account_id || account.id }}</span>
      </div>

      <div class="delete-account-copy">
        <p>将删除账号库记录、该账号保存的 auth.json / original.json、可选 config.toml 和额度状态记录。</p>
        <p>不会删除当前 Codex home 中正在使用的 auth.json 或 config.toml。</p>
      </div>

      <label class="checkbox-row delete-profile-option">
        <input
          :checked="deleteProfile"
          type="checkbox"
          :disabled="deleting || !account.browser_profile_dir"
          @change="$emit('update:deleteProfile', checkedValue($event))"
        />
        <span>
          同时删除该账号的登录缓存
          <small>{{ account.browser_profile_dir ? "包括登录窗口保存的 cookie、缓存和本地存储（浏览器 Profile）" : "该账号没有记录登录缓存目录" }}</small>
        </span>
      </label>

      <div class="modal-actions">
        <button class="secondary" type="button" :disabled="deleting" @click="$emit('cancel')">取消</button>
        <button class="danger-action" type="button" :disabled="deleting" @click="$emit('confirm')">
          {{ deleting ? "正在删除" : "删除" }}
        </button>
      </div>
    </section>
  </div>
</template>
