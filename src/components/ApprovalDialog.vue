<script setup lang="ts">
import type { Approval } from '../types';

defineProps<{ approvals: Approval[] }>();
const emit = defineEmits<{ respond: [a: { id: string; approved: boolean }] }>();

const toolLabel: Record<string, string> = {
  write_file: '写入文件',
  run_command: '执行命令',
};

function short(a: Approval) {
  const label = toolLabel[a.tool] ?? a.tool;
  const summary = (a.summary || '').replace(/\s+/g, ' ').slice(0, 28);
  return summary ? `${label}：${summary}${a.summary.length > 28 ? '…' : ''}` : label;
}
</script>

<template>
  <div v-if="approvals.length" class="modal-mask approval-mask" role="dialog" aria-modal="true" aria-label="操作审批">
    <div class="modal">
      <h3>⚠️ 操作需要批准</h3>
      <p>AI 助手请求执行以下操作，请逐条确认是否允许：</p>

      <div v-for="a in approvals" :key="a.id" class="approval-row">
        <div class="approval-summary">{{ a.summary }}</div>
        <div class="approval-row-actions">
          <button class="btn btn-danger btn-sm" @click="emit('respond', { id: a.id, approved: false })">
            拒绝
          </button>
          <button class="btn btn-primary btn-sm" @click="emit('respond', { id: a.id, approved: true })">
            允许
          </button>
        </div>
        <div class="approval-what">{{ short(a) }}</div>
      </div>

      <div v-if="approvals.length > 1" class="modal-actions">
        <button class="btn" @click="approvals.forEach((a) => emit('respond', { id: a.id, approved: false }))">全部拒绝</button>
        <button class="btn" @click="approvals.forEach((a) => emit('respond', { id: a.id, approved: true }))">全部允许</button>
      </div>
      <p style="font-size:11px;margin-top:14px;">提示：可在「模型设置 → 工具策略」中改为自动放行或禁用。</p>
    </div>
  </div>
</template>
