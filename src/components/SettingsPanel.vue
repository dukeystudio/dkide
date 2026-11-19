<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { useChatStore } from '../store';
import { api } from '../api';
import type { AppConfig, Provider } from '../types';

const store = useChatStore();
const result = ref('');
const testing = ref(false);
const saved = ref(true);
// 使用本地草稿，避免未点保存就改到内存配置导致前后端静默分叉
const draft = ref<AppConfig | null>(null);

const config = computed(() => draft.value);

watch(() => store.config, (c) => {
  if (c && (!draft.value || saved.value)) draft.value = JSON.parse(JSON.stringify(c));
}, { immediate: true });

watch(draft, () => { saved.value = false; }, { deep: true });

const presets = [
  { name: 'OpenAI', url: 'https://api.openai.com/v1', model: 'gpt-4o-mini' },
  { name: 'DeepSeek', url: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
  { name: 'Moonshot', url: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k' },
  { name: '智谱', url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4' },
];

function addProvider(preset?: typeof presets[0]) {
  if (!config.value) return;
  config.value.providers.push({
    id: crypto.randomUUID(),
    name: preset?.name ?? '新中转站',
    base_url: preset?.url ?? 'https://api.openai.com/v1',
    api_key: '',
    model: preset?.model ?? 'gpt-4o-mini',
  });
}

function removeProvider(id: string) {
  if (!config.value || config.value.providers.length <= 1) return;
  config.value.providers = config.value.providers.filter((p) => p.id !== id);
  if (config.value.active_provider_id === id) {
    config.value.active_provider_id = config.value.providers[0].id;
  }
}

async function save() {
  if (!config.value) return;
  const next = JSON.parse(JSON.stringify(config.value)) as AppConfig;
  await store.saveConfig(next);
  saved.value = true;
  result.value = '设置已保存';
}

async function test(p: Provider) {
  testing.value = true;
  result.value = '测试中…';
  try {
    result.value = await api.testProvider(p);
  } catch (e: any) {
    result.value = `失败: ${typeof e === 'string' ? e : e?.message ?? e}`;
  } finally { testing.value = false; }
}

async function pickWorkspace() {
  if (!config.value) return;
  const path = await open({ directory: true, multiple: false });
  if (path && typeof path === 'string') config.value.workspace_path = path;
}
</script>

<template>
  <div class="panel-page">
    <div class="panel-inner">
      <div style="display:flex;justify-content:space-between;align-items:center;">
        <h2>模型设置</h2>
        <span v-if="!saved" class="tag" style="background:var(--warn-bg);color:var(--warn-text);">● 有未保存的更改</span>
      </div>
      <p style="color:var(--text-muted);margin-top:0;">配置 OpenAI 兼容中转站，可添加多个并切换。</p>

      <template v-if="config">
        <div v-for="p in config.providers" :key="p.id" class="provider-item" :class="{ active: config.active_provider_id === p.id }">
          <div class="provider-head">
            <label style="display:flex;align-items:center;gap:8px;font-weight:600;">
              <input type="radio" :value="p.id" v-model="config.active_provider_id" name="active" />
              {{ p.name }}
            </label>
            <button class="btn btn-danger btn-sm" @click="removeProvider(p.id)">删除</button>
          </div>
          <div class="row">
            <div class="field"><label>名称</label><input v-model="p.name" /></div>
            <div class="field"><label>模型名称</label><input v-model="p.model" placeholder="gpt-4o-mini" /></div>
          </div>
          <div class="field"><label>Base URL（以 /v1 结尾）</label><input v-model="p.base_url" /></div>
          <div class="field"><label>API Key（存入系统钥匙串）</label><input v-model="p.api_key" type="password" placeholder="sk-..." /></div>
          <button class="btn btn-sm" :disabled="testing" @click="test(p)">测试连接</button>
        </div>

        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:18px;">
          <button v-for="p in presets" :key="p.name" class="btn btn-sm" @click="addProvider(p)">+ {{ p.name }}</button>
          <button class="btn btn-sm" @click="addProvider()">+ 自定义</button>
        </div>

        <div class="card">
          <div class="card-title">工具工作目录</div>
          <p style="color:var(--text-muted);font-size:12px;margin-top:0;">文件工具与该目录交互，路径越界会被拒绝。留空则使用应用数据目录。</p>
          <div style="display:flex;gap:8px;">
            <input v-model="config.workspace_path" placeholder="留空使用应用数据目录" class="inline-input" />
            <button class="btn" @click="pickWorkspace">选择…</button>
          </div>
        </div>

        <div class="card">
          <div class="card-title">工具策略与安全</div>
          <div class="field">
            <label>命令/写入操作的审批方式</label>
            <select v-model="config.tool_policy">
              <option value="ask">每次询问（推荐）</option>
              <option value="auto">自动放行</option>
              <option value="deny">禁用危险工具</option>
            </select>
          </div>
          <div class="row">
            <div class="field">
              <label>上下文 token 预算</label>
              <input v-model.number="config.token_budget" type="number" min="2000" step="1000" />
            </div>
            <div class="field">
              <label>自定义单价（$/百万 token，0=用内置表）</label>
              <input v-model.number="config.cost_per_million" type="number" min="0" step="0.1" />
            </div>
          </div>
          <div class="field">
            <label>技能市场索引 URL（留空使用内置推荐）</label>
            <input v-model="config.marketplace_url" placeholder="https://example.com/skills/index.json" />
          </div>
        </div>

        <div class="card">
          <div class="card-title">外观</div>
          <div class="field">
            <label>主题</label>
            <select v-model="config.theme">
              <option value="light">浅色</option>
              <option value="dark">深色</option>
              <option value="system">跟随系统</option>
            </select>
          </div>
        </div>

        <div class="card">
          <div class="card-title">全局系统提示词</div>
          <div class="field"><textarea v-model="config.system_prompt"></textarea></div>
        </div>

        <div style="display:flex;gap:10px;align-items:center;flex-wrap:wrap;">
          <button class="btn btn-primary" @click="save">保存设置</button>
          <span v-if="result" class="tag">{{ result }}</span>
        </div>
      </template>
    </div>
  </div>
</template>
