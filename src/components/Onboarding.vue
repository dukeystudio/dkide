<script setup lang="ts">
import { ref } from 'vue';
import { useChatStore } from '../store';
import { api } from '../api';
import type { Provider } from '../types';

const emit = defineEmits<{ done: [] }>();
const store = useChatStore();
const name = ref('我的中转站');
const baseUrl = ref('https://api.openai.com/v1');
const apiKey = ref('');
const model = ref('gpt-4o-mini');
const testing = ref(false);
const message = ref('');
const error = ref('');

const presets = [
  { name: 'OpenAI', url: 'https://api.openai.com/v1', model: 'gpt-4o-mini' },
  { name: 'DeepSeek', url: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
  { name: 'Moonshot', url: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k' },
];
function applyPreset(p: typeof presets[0]) {
  name.value = p.name; baseUrl.value = p.url; model.value = p.model;
}

async function test() {
  testing.value = true; message.value = ''; error.value = '';
  try {
    message.value = await api.testProvider({
      id: 'test', name: name.value, base_url: baseUrl.value, api_key: apiKey.value, model: model.value,
    });
  } catch (e: any) {
    error.value = typeof e === 'string' ? e : e?.message ?? String(e);
  } finally { testing.value = false; }
}

async function finish() {
  if (!apiKey.value.trim()) { error.value = '请先填写 API Key'; return; }
  if (!store.config) return;
  const provider: Provider = {
    id: crypto.randomUUID(), name: name.value, base_url: baseUrl.value,
    api_key: apiKey.value, model: model.value,
  };
  store.config.providers = [provider];
  store.config.active_provider_id = provider.id;
  await store.saveConfig(store.config);
  emit('done');
}
</script>

<template>
  <div class="onboarding">
    <div class="onboard-card">
      <div class="brand-hero"><span class="brand-dot"></span><div>
        <h1>欢迎使用 都客AI助手</h1>
        <div class="subtitle" style="margin:0;">dkide · 配置一个模型中转站即可开始</div>
      </div></div>

      <div class="onboard-step">
        <div class="step-num">1</div>
        <div style="flex:1;">
          <div style="font-weight:600;margin-bottom:8px;">选择服务商或手动填写</div>
          <div style="display:flex;gap:8px;margin-bottom:14px;flex-wrap:wrap;">
            <button v-for="p in presets" :key="p.name" class="btn btn-sm" @click="applyPreset(p)">{{ p.name }}</button>
          </div>
          <div class="field"><label>名称</label><input v-model="name" /></div>
          <div class="field"><label>Base URL（OpenAI 兼容端点）</label><input v-model="baseUrl" /></div>
          <div class="row">
            <div class="field"><label>模型名称</label><input v-model="model" /></div>
            <div class="field"><label>API Key</label><input v-model="apiKey" type="password" placeholder="sk-..." /></div>
          </div>
        </div>
      </div>

      <div class="onboard-step">
        <div class="step-num">2</div>
        <div style="flex:1;">
          <div style="font-weight:600;margin-bottom:8px;">测试并进入</div>
          <div style="display:flex;gap:10px;align-items:center;flex-wrap:wrap;">
            <button class="btn" :disabled="testing" @click="test">{{ testing ? '测试中…' : '测试连接' }}</button>
            <button class="btn btn-primary" @click="finish">保存并进入</button>
            <span v-if="message" class="toast">{{ message }}</span>
          </div>
          <div v-if="error" class="error-box" style="margin-top:10px;margin-bottom:0;">{{ error }}</div>
          <p style="font-size:12px;margin-top:12px;">API Key 会存入系统钥匙串，不会明文写入配置文件。之后可在「模型设置」中管理。</p>
          <p style="font-size:11px;margin-top:6px;color:var(--text-faint);">都客AI助手 · dkide</p>
        </div>
      </div>
    </div>
  </div>
</template>
