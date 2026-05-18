<script setup lang="ts">
import type { BackupSummary } from "../types";
import { formatDate } from "../utils/format";

defineProps<{
  backups: BackupSummary[];
  busy: boolean;
}>();

defineEmits<{
  restoreBackup: [BackupSummary];
}>();
</script>

<template>
  <section class="panel">
    <div v-if="!backups.length" class="empty">
      <strong>暂无备份</strong>
      <p>每次切换账号前都会自动备份，也可以手动创建。</p>
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
        <button class="secondary" :disabled="busy" @click="$emit('restoreBackup', backup)">恢复</button>
      </article>
    </div>
  </section>
</template>
