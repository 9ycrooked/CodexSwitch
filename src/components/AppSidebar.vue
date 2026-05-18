<script setup lang="ts">
import type { CodexState } from "../types";

type Tab = "accounts" | "quota" | "backups" | "settings";

defineProps<{
  selectedTab: Tab;
  current: CodexState | null;
}>();

defineEmits<{
  select: [Tab];
}>();
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">C</div>
      <div>
        <h1>Codex Switch</h1>
        <p>本地 OAuth 凭据管理</p>
      </div>
    </div>

    <nav class="tabs" aria-label="主导航">
      <button :class="{ active: selectedTab === 'accounts' }" @click="$emit('select', 'accounts')">
        <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z" />
          <path d="M4 21a8 8 0 0 1 16 0" />
        </svg>
        <span>账号</span>
      </button>
      <button :class="{ active: selectedTab === 'quota' }" @click="$emit('select', 'quota')">
        <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 19V5" />
          <path d="M4 19h16" />
          <path d="M8 16v-5" />
          <path d="M12 16V8" />
          <path d="M16 16v-3" />
        </svg>
        <span>额度</span>
      </button>
      <button :class="{ active: selectedTab === 'backups' }" @click="$emit('select', 'backups')">
        <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 3v7l4 2" />
          <path d="M20 12a8 8 0 1 1-2.35-5.65" />
          <path d="M20 4v5h-5" />
        </svg>
        <span>备份</span>
      </button>
      <button :class="{ active: selectedTab === 'settings' }" @click="$emit('select', 'settings')">
        <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.05.05a2 2 0 0 1-2.83 2.83l-.05-.05a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.08a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.05.05a2 2 0 0 1-2.83-2.83l.05-.05A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.08a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.05-.05a2 2 0 0 1 2.83-2.83l.05.05A1.65 1.65 0 0 0 8.9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.08a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.05-.05a2 2 0 0 1 2.83 2.83l-.05.05A1.65 1.65 0 0 0 19.4 9c.14.47.51.84.98.98H21a2 2 0 0 1 0 4h-.08a1.65 1.65 0 0 0-1.52 1Z" />
        </svg>
        <span>设置</span>
      </button>
    </nav>

    <section class="status-panel">
      <span class="eyebrow">当前 Codex</span>
      <strong>{{ current?.current_email || current?.current_account_id || "未识别账号" }}</strong>
      <small>{{ current?.codex_home }}</small>
      <div class="state-row">
        <span :class="['dot', current?.auth_exists ? 'ok' : 'bad']"></span>
        auth.json
        <span :class="['dot', current?.config_exists ? 'ok' : 'bad']"></span>
        config.toml
      </div>
    </section>
  </aside>
</template>
