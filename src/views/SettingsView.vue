<script setup lang="ts">
import type { AppPaths, AutoFlowOAuthServerStatus, NetworkExitCheckResult, Settings } from "../types";
import { formatDate } from "../utils/format";

defineProps<{
  settings: Settings;
  busy: boolean;
  appVersion: string;
  appPaths: AppPaths | null;
  lastUpdateCheckedAt: string | null;
  updateChecking: boolean;
  updateDownloading: boolean;
  updatePolicySource: string;
  updatePolicyError: string;
  networkCheckResult: NetworkExitCheckResult | null;
  networkCheckRunning: boolean;
  autoflowServerStatus: AutoFlowOAuthServerStatus | null;
  autoflowServerBusy: boolean;
}>();

defineEmits<{
  updateProcessNames: [Event];
  checkForUpdates: [];
  checkNetworkExit: [];
  startAutoflowServer: [];
  stopAutoflowServer: [];
  resetAutoflowAdminKey: [];
  copyAutoflowServiceUrl: [];
  copyAutoflowAdminKey: [];
  saveSettings: [];
  openCodexHome: [];
  openAppData: [];
  openProfiles: [];
}>();

function statusText(status: string) {
  if (status === "ok") return "正常";
  if (status === "warning") return "警告";
  if (status === "failed") return "失败";
  return status;
}

function maskedKey(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return "尚未生成";
  if (trimmed.length <= 12) return "••••••••";
  return `${trimmed.slice(0, 6)}••••••${trimmed.slice(-4)}`;
}
</script>

<template>
  <section class="panel settings">
    <section class="settings-info-grid">
      <article class="info-card">
        <span class="eyebrow">App</span>
        <h3>应用信息</h3>
        <p>当前版本：{{ appVersion || "读取中" }}</p>
        <p>最近检查：{{ lastUpdateCheckedAt ? formatDate(lastUpdateCheckedAt) : "从未检查" }}</p>
      </article>
      <article class="info-card">
        <span class="eyebrow">Storage</span>
        <h3>数据位置</h3>
        <p>应用数据：{{ appPaths?.app_store_dir || "读取中" }}</p>
        <div class="inline-actions">
          <button class="secondary" type="button" :disabled="busy" @click="$emit('openAppData')">打开应用数据</button>
        </div>
      </article>
    </section>
    <label class="form-group">
      <span>Codex home</span>
      <div class="field-with-action">
        <input v-model="settings.codex_home" />
        <button class="secondary" type="button" :disabled="busy" @click="$emit('openCodexHome')">打开</button>
      </div>
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
      <div class="field-with-action">
        <input v-model="settings.browser_profile_dir" />
        <button class="secondary" type="button" :disabled="busy" @click="$emit('openProfiles')">打开</button>
      </div>
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
    <section class="autoflow-integration-panel">
      <div class="panel-heading-row">
        <div>
          <span class="eyebrow">AutoFlow</span>
          <h3>自有软件 OAuth 接入服务</h3>
          <p>按需开启本地接口，让 AutoFlow 用 Codex2API 协议添加 OAuth 账号。</p>
        </div>
        <span class="service-state" :class="{ running: autoflowServerStatus?.running }">
          {{ autoflowServerStatus?.running ? "运行中" : "未开启" }}
        </span>
      </div>
      <label class="form-group">
        <span>接入服务端口</span>
        <input
          v-model.number="settings.autoflow_oauth_server_port"
          type="number"
          min="1024"
          max="65535"
          :disabled="autoflowServerStatus?.running || autoflowServerBusy"
        />
      </label>
      <div class="service-field-row">
        <span>AutoFlow 地址</span>
        <code>{{ autoflowServerStatus?.url || `http://127.0.0.1:${settings.autoflow_oauth_server_port}/admin/accounts` }}</code>
        <button class="secondary" type="button" :disabled="autoflowServerBusy" @click="$emit('copyAutoflowServiceUrl')">
          复制
        </button>
      </div>
      <div class="service-field-row">
        <span>管理密钥</span>
        <code>{{ maskedKey(settings.autoflow_oauth_admin_key) }}</code>
        <button
          class="secondary"
          type="button"
          :disabled="autoflowServerBusy || !settings.autoflow_oauth_admin_key"
          @click="$emit('copyAutoflowAdminKey')"
        >
          复制
        </button>
        <button class="secondary" type="button" :disabled="autoflowServerBusy" @click="$emit('resetAutoflowAdminKey')">
          重置
        </button>
      </div>
      <div class="service-actions">
        <button
          type="button"
          :disabled="busy || autoflowServerBusy || autoflowServerStatus?.running"
          @click="$emit('startAutoflowServer')"
        >
          {{ autoflowServerBusy ? "处理中" : "开启接入服务" }}
        </button>
        <button
          class="secondary"
          type="button"
          :disabled="busy || autoflowServerBusy || !autoflowServerStatus?.running"
          @click="$emit('stopAutoflowServer')"
        >
          关闭接入服务
        </button>
      </div>
    </section>
    <section class="network-check-panel">
      <div class="panel-heading-row">
        <div>
          <span class="eyebrow">OAuth Network</span>
          <h3>登录前网络检查</h3>
          <p>检查软件后端是否能访问 OpenAI OAuth 服务；出口地区查询默认关闭。</p>
        </div>
        <button class="secondary" type="button" :disabled="busy || networkCheckRunning" @click="$emit('checkNetworkExit')">
          {{ networkCheckRunning ? "检查中" : "立即检查" }}
        </button>
      </div>
      <label class="checkbox-row">
        <input v-model="settings.check_oauth_network_on_login" type="checkbox" />
        <span>OAuth 登录前自动检查后端网络</span>
      </label>
      <label class="checkbox-row">
        <input v-model="settings.check_egress_region" type="checkbox" />
        <span>显示后端出口 IP 和国家代码（使用 Cloudflare trace）</span>
      </label>
      <div v-if="networkCheckResult" class="network-check-result">
        <p>最近结果：{{ networkCheckResult.overall_status }}</p>
        <dl>
          <div>
            <dt>整体状态</dt>
            <dd>{{ statusText(networkCheckResult.overall_status) }}</dd>
          </div>
          <div v-if="networkCheckResult.backend_country">
            <dt>出口国家</dt>
            <dd>{{ networkCheckResult.backend_country }}</dd>
          </div>
          <div v-if="networkCheckResult.backend_ip">
            <dt>出口 IP</dt>
            <dd>{{ networkCheckResult.backend_ip }}</dd>
          </div>
          <div v-if="networkCheckResult.auth_status != null">
            <dt>OAuth HTTP</dt>
            <dd>{{ networkCheckResult.auth_status }}</dd>
          </div>
        </dl>
        <p v-if="networkCheckResult.errors.length">
          错误：{{ networkCheckResult.errors.join("；") }}
        </p>
        <p v-if="networkCheckResult.warnings.length">
          警告：{{ networkCheckResult.warnings.join("；") }}
        </p>
        <small v-if="networkCheckResult.backend_country">
          出口地区仅供参考，OpenAI token exchange 的最终判定可能不同。
        </small>
      </div>
    </section>
    <label class="checkbox-row">
      <input v-model="settings.keep_login_profiles" type="checkbox" />
      <span>保留登录 Profile，用于隔离并复用该账号的浏览器会话</span>
    </label>
    <section class="update-settings-panel">
      <div>
        <span class="eyebrow">Updater</span>
        <h3>更新检查</h3>
        <small>当前策略：{{ updatePolicySource }}</small>
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
      当前版本使用明文保存账号凭据。WebView2 Profile 只是隔离 cookie/cache/localStorage，不伪造设备指纹；账号库包含 refresh token，请不要共享应用数据目录或导出的账号文件。
    </div>
    <button :disabled="busy" @click="$emit('saveSettings')">保存设置</button>
  </section>
</template>
