<script setup lang="ts">
import { ref } from "vue";
import type { AppPaths, AutoFlowOAuthServerStatus, NetworkExitCheckResult, Settings } from "../types";
import { formatDate } from "../utils/format";

type SettingsSection = "basic" | "login" | "integrations" | "updates" | "security" | "about";

defineProps<{
  settings: Settings;
  settingsDirty: boolean;
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
  openProjectRepository: [];
  openProjectIssues: [];
  copySupportEmail: [];
  openSupportEmail: [];
}>();

const selectedSettingsSection = ref<SettingsSection>("basic");
const settingsSections: Array<{ id: SettingsSection; label: string; summary: string }> = [
  { id: "basic", label: "基础", summary: "Codex 目录与进程" },
  { id: "login", label: "登录", summary: "OAuth 与登录缓存" },
  { id: "integrations", label: "集成", summary: "AutoFlow 接入" },
  { id: "updates", label: "更新", summary: "版本检查" },
  { id: "security", label: "安全", summary: "凭据与数据位置" },
  { id: "about", label: "关于", summary: "项目与反馈" }
];

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
  <section class="panel settings settings-page">
    <nav class="settings-nav" aria-label="设置分类">
      <button
        v-for="section in settingsSections"
        :key="section.id"
        type="button"
        :class="{ active: selectedSettingsSection === section.id }"
        @click="selectedSettingsSection = section.id"
      >
        <strong>{{ section.label }}</strong>
        <span>{{ section.summary }}</span>
      </button>
    </nav>

    <section v-if="selectedSettingsSection === 'basic'" class="settings-section">
      <header class="settings-section-header">
        <span class="eyebrow">Basic</span>
        <h3>基础配置</h3>
        <p>管理 Codex 配置目录与进程关闭名单。</p>
      </header>
      <div class="settings-section-grid">
        <section class="settings-info-grid settings-section-card full">
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
          <span>Codex 配置目录</span>
          <div class="field-with-action">
            <input v-model="settings.codex_home" />
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openCodexHome')">打开</button>
          </div>
        </label>
        <label class="form-group">
          <span>需要关闭的 Codex 进程</span>
          <input :value="settings.process_names.join(', ')" @input="$emit('updateProcessNames', $event)" />
          <small>多个进程名用英文逗号分隔。</small>
        </label>
      </div>
    </section>

    <section v-else-if="selectedSettingsSection === 'login'" class="settings-section">
      <header class="settings-section-header">
        <span class="eyebrow">OAuth</span>
        <h3>登录与隔离</h3>
        <p>配置 OAuth 登录方式、回调端口和每个账号的登录缓存目录。</p>
      </header>
      <div class="settings-section-grid">
        <label class="form-group">
          <span>OAuth 登录方式</span>
          <select v-model="settings.oauth_login_mode">
            <option value="external">外部隔离浏览器（推荐）</option>
            <option value="embedded">内置 WebView2（实验）</option>
          </select>
        </label>
        <label class="form-group">
          <span>登录回调端口</span>
          <input v-model.number="settings.oauth_callback_port" type="number" min="1024" max="65535" />
        </label>
        <label class="form-group full">
          <span>登录缓存目录</span>
          <div class="field-with-action">
            <input v-model="settings.browser_profile_dir" />
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openProfiles')">打开</button>
          </div>
          <small>这里保存 OAuth 登录窗口的 cookie、缓存和本地存储，用于账号隔离。</small>
        </label>
        <label class="checkbox-row full">
          <input v-model="settings.keep_login_profiles" type="checkbox" />
          <span>保留账号登录缓存，便于下次继续使用该账号的登录会话</span>
        </label>

        <section class="network-check-panel">
          <div class="panel-heading-row">
            <div>
              <span class="eyebrow">Login network</span>
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
      </div>
    </section>

    <section v-else-if="selectedSettingsSection === 'integrations'" class="settings-section">
      <header class="settings-section-header">
        <span class="eyebrow">Integrations</span>
        <h3>外部工具接入</h3>
        <p>按需开启本地 AutoFlow OAuth 接入服务。</p>
      </header>
      <section class="autoflow-integration-panel">
        <div class="panel-heading-row">
          <div>
            <span class="eyebrow">AutoFlow</span>
            <h3>OAuth 接入服务</h3>
            <p>通过 Codex2API 协议添加 OAuth 账号。</p>
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
    </section>

    <section v-else-if="selectedSettingsSection === 'updates'" class="settings-section">
      <header class="settings-section-header">
        <span class="eyebrow">Updater</span>
        <h3>更新检查</h3>
        <p>启动检查和强制更新策略由发布包的 update-policy.json 控制，这里只保留手动检查入口。</p>
      </header>
      <section class="update-settings-panel">
        <div>
          <span class="eyebrow">Current policy</span>
          <h3>当前更新策略</h3>
          <p>当前策略：{{ updatePolicySource }}</p>
          <small v-if="updatePolicyError">发布配置读取失败：{{ updatePolicyError }}</small>
          <small v-else>最近检查：{{ lastUpdateCheckedAt ? formatDate(lastUpdateCheckedAt) : "从未检查" }}</small>
        </div>
        <button
          class="secondary"
          type="button"
          :disabled="busy || updateChecking || updateDownloading"
          @click="$emit('checkForUpdates')"
        >
          {{ updateChecking ? "检查中" : "检查更新" }}
        </button>
      </section>
    </section>

    <section v-else-if="selectedSettingsSection === 'security'" class="settings-section">
      <header class="settings-section-header">
        <span class="eyebrow">Security</span>
        <h3>凭据与数据</h3>
        <p>查看账号库和登录缓存的存储位置，确认本地凭据管理风险。</p>
      </header>
      <section class="settings-info-grid">
        <article class="info-card">
          <span class="eyebrow">Account store</span>
          <h3>账号库目录</h3>
          <p>{{ appPaths?.app_store_dir || "读取中" }}</p>
          <div class="inline-actions">
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openAppData')">打开应用数据</button>
          </div>
        </article>
        <article class="info-card">
          <span class="eyebrow">Login cache</span>
          <h3>登录缓存目录</h3>
          <p>{{ settings.browser_profile_dir || appPaths?.browser_profile_dir || "读取中" }}</p>
          <div class="inline-actions">
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openProfiles')">打开登录缓存</button>
          </div>
        </article>
      </section>
      <div class="warning">
        当前版本使用明文保存账号凭据。登录缓存只隔离 cookie/cache/localStorage；账号库包含 refresh token，请不要共享应用数据目录或导出的账号文件。
      </div>
    </section>

    <section v-else class="settings-section">
      <header class="settings-section-header">
        <span class="eyebrow">About</span>
        <h3>关于 Codex Switch</h3>
        <p>查看项目仓库、提交反馈，或通过邮箱联系维护者。</p>
      </header>

      <section class="settings-info-grid about-grid">
        <article class="info-card about-card">
          <div>
            <span class="eyebrow">Repository</span>
            <h3>项目仓库</h3>
            <p>查看源码、发布记录和项目说明。</p>
          </div>
          <code>github.com/9ycrooked/CodexSwitch</code>
          <div class="inline-actions">
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openProjectRepository')">
              打开仓库
            </button>
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openProjectIssues')">
              提交 Issue
            </button>
          </div>
        </article>

        <article class="info-card about-card">
          <div>
            <span class="eyebrow">Contact</span>
            <h3>问题反馈</h3>
            <p>Bug、功能建议和使用问题都可以通过邮箱联系。</p>
          </div>
          <code>qianmang1@gmail.com</code>
          <div class="inline-actions">
            <button class="secondary" type="button" :disabled="busy" @click="$emit('copySupportEmail')">
              复制邮箱
            </button>
            <button class="secondary" type="button" :disabled="busy" @click="$emit('openSupportEmail')">
              写邮件
            </button>
          </div>
        </article>
      </section>
    </section>

    <div v-if="settingsDirty" class="settings-save-bar" role="status">
      <div>
        <strong>设置已修改</strong>
        <span>保存后才会应用到后续登录、切换和本地服务。</span>
      </div>
      <button type="button" :disabled="busy" @click="$emit('saveSettings')">
        {{ busy ? "保存中" : "保存设置" }}
      </button>
    </div>
  </section>
</template>
