<script setup lang="ts">
import type { Settings } from "../types";

defineProps<{
  settings: Settings;
  busy: boolean;
  updateChecking: boolean;
  updateDownloading: boolean;
  updatePolicySource: string;
  updatePolicyError: string;
}>();

defineEmits<{
  updateProcessNames: [Event];
  checkForUpdates: [];
  saveSettings: [];
}>();
</script>

<template>
  <section class="panel settings">
    <label class="form-group">
      <span>Codex home</span>
      <input v-model="settings.codex_home" />
    </label>
    <label class="form-group">
      <span>Codex 进程名</span>
      <input :value="settings.process_names.join(', ')" @input="$emit('updateProcessNames', $event)" />
    </label>
    <label class="form-group">
      <span>关闭超时（毫秒）</span>
      <input v-model.number="settings.close_timeout_ms" type="number" min="1000" step="500" />
    </label>
    <label class="form-group">
      <span>WebView2 Profile 目录</span>
      <input v-model="settings.browser_profile_dir" />
    </label>
    <label class="form-group">
      <span>OAuth callback 端口</span>
      <input v-model.number="settings.oauth_callback_port" type="number" min="1024" max="65535" />
    </label>
    <label class="form-group">
      <span>OAuth 登录方式</span>
      <select v-model="settings.oauth_login_mode">
        <option value="external">外部隔离浏览器（推荐）</option>
        <option value="embedded">内置 WebView2（实验）</option>
      </select>
    </label>
    <label class="checkbox-row">
      <input v-model="settings.keep_login_profiles" type="checkbox" />
      <span>保留登录 Profile，用于隔离并复用该账号的浏览器会话</span>
    </label>
    <section class="update-settings-panel">
      <div>
        <span class="eyebrow">Updater</span>
        <h3>更新检查</h3>
        <p>
          启动检查和强制更新策略由发布包里的 update-policy.json 控制。默认策略为启动时检查更新，发现新版本时询问是否更新。
        </p>
        <small>当前策略：{{ updatePolicySource }}</small>
        <small v-if="updatePolicyError">{{ updatePolicyError }}</small>
      </div>
      <button
        class="secondary"
        :disabled="busy || updateChecking || updateDownloading"
        @click="$emit('checkForUpdates')"
      >
        {{ updateChecking ? "检查中" : "检查更新" }}
      </button>
    </section>
    <div class="warning">
      当前版本按你的要求使用明文保存账号凭据。WebView2 Profile 只是隔离 cookie/cache/localStorage，不伪造设备指纹；账号库包含 refresh token，请不要共享应用数据目录或导出的账号文件。
    </div>
    <button :disabled="busy" @click="$emit('saveSettings')">保存设置</button>
  </section>
</template>
