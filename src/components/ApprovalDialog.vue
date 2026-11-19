<script setup lang="ts">
import type { Approval } from '../types';

defineProps<{ approvals: Approval[] }>();
const emit = defineEmits<{ respond: [a: { id: string; approved: boolean }] }>();

const toolLabel: Record<string, string> = {
  write_file: '写入文件',
  run_command: '执行命令',
};
</script>

<template>
  <div v-if="approvals.length" class="modal-mask">
    <div class="modal">
      <h3>⚠️ 操作需要批准</h3>
      <p>AI 助手请求执行以下操作，请确认是否允许：</p>
      <div v-for="a in approvals" :key="a.id" style="margin-bottom:14px;">
        <div style="font-weight:600;margin-bottom:6px;">{{ toolLabel[a.tool] ?? a.tool }}</div>
        <div class="approval-summary">{{ a.summary }}</div>
      </div>
      <div class="modal-actions">
        <button v-for="a in approvals" :key="'d-' + a.id" class="btn btn-danger" @click="emit('respond', { id: a.id, approved: false })">拒绝</button>
        <button v-for="a in approvals" :key="'a-' + a.id" class="btn btn-primary" @click="emit('respond', { id: a.id, approved: true })">允许</button>
      </div>
      <p style="font-size:11px;margin-top:14px;">提示：可在「模型设置 → 工具策略」中改为自动放行或禁用。</p>
    </div>
  </div>
</template>
