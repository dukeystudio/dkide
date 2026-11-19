<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { api } from '../api';

const info = ref({ name: '都客AI助手', english_name: 'dkide', version: '1.0.0', description: '' });
onMounted(async () => {
  try { info.value = await api.getAppInfo(); } catch { /* 使用默认值 */ }
});
</script>

<template>
  <div class="panel-page">
    <div class="panel-inner">
      <div style="text-align:center;padding:20px 0 32px;">
        <div class="brand-dot" style="width:56px;height:56px;border-radius:16px;margin:0 auto 16px;"></div>
        <h1 style="margin:0;font-size:26px;">{{ info.name }}</h1>
        <p style="color:var(--text-muted);">{{ info.english_name }} · 版本 {{ info.version }}</p>
      </div>

      <div class="card">
        <div class="card-title">关于</div>
        <p style="color:var(--text-muted);line-height:1.8;">
          都客AI助手（dkide）是一款本地优先的桌面 AI 助手，支持接入 OpenAI 兼容的模型中转站，
          通过技能（Skills）与内置工具扩展能力。你的对话数据与配置均存储在本地，
          API Key 保存于系统钥匙串。
        </p>
      </div>

      <div class="card">
        <div class="card-title">隐私与安全</div>
        <ul style="color:var(--text-muted);line-height:1.9;padding-left:20px;">
          <li>对话历史与配置保存在本机应用数据目录，不上传云端</li>
          <li>API Key 存于系统钥匙串，配置文件中不包含明文密钥</li>
          <li>文件工具限制在指定工作目录内，阻止路径穿越</li>
          <li>命令工具支持黑名单、超时与用户审批，危险操作需手动确认</li>
          <li>技能 ZIP 安装经过 Zip Slip 路径校验</li>
        </ul>
      </div>

      <div class="card">
        <div class="card-title">技术栈</div>
        <p style="color:var(--text-muted);">Rust · Tauri 2 · Vue 3 · TypeScript</p>
      </div>

      <p style="text-align:center;color:var(--text-faint);font-size:12px;margin-top:24px;">
        © 2026 都客AI助手 · dkide · 本地优先，安全可控
      </p>
    </div>
  </div>
</template>
