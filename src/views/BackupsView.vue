<script setup lang="ts">
import type { BackupSummary } from "../types";
import { formatDate } from "../utils/format";

defineProps<{
  backups: BackupSummary[];
  busy: boolean;
  isOperationActive: (key: string) => boolean;
  hasActiveOperation: boolean;
}>();

defineEmits<{
  restoreBackup: [BackupSummary];
  openBackups: [];
  openBackup: [BackupSummary];
}>();
</script>

<template>
  <section class="panel">
    <div v-if="!backups.length" class="empty">
      <strong>暂无备份</strong>
      <p>每次切换账号前都会自动备份，也可以手动创建。</p>
      <button class="secondary" type="button" :disabled="busy" @click="$emit('openBackups')">
        打开备份目录
      </button>
    </div>
    <div v-else class="backup-list">
      <article v-for="backup in backups" :key="backup.id" class="backup-row card-row">
        <div>
          <h3>{{ backup.id }}</h3>
          <p>{{ formatDate(backup.created_at) }}</p>
          <small>
            auth: {{ backup.auth_path ? "有" : "无" }} · config:
            {{ backup.config_path ? "有" : "无" }}
          </small>
        </div>
        <div class="row-actions">
          <button
            class="secondary"
            type="button"
            :disabled="busy || isOperationActive('open:backup:' + backup.id)"
            @click="$emit('openBackup', backup)"
          >
            打开目录
          </button>
          <button
            class="secondary"
            :disabled="busy || isOperationActive('backup:restore:' + backup.id)"
            @click="$emit('restoreBackup', backup)"
          >
            恢复
          </button>
        </div>
      </article>
    </div>
  </section>
</template>
