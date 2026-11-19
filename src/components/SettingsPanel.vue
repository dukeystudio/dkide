<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { useChatStore } from '../store';
import { useThemeStore, ACCENTS, THEME_LABELS } from '../themeStore';
import type { AppConfig } from '../types';

const theme = useThemeStore();
const accents = Object.keys(ACCENTS) as Array<keyof typeof ACCENTS>;

const store = useChatStore();
const result = ref('');
const saved = ref(true);
// 使用本地草稿，避免未点保存就改到内存配置导致前后端静默分叉
const draft = ref<AppConfig | null>(null);

const config = computed(() => draft.value);

watch(() => store.config, (c) => {
  if (c && (!draft.value || saved.value)) draft.value = JSON.parse(JSON.stringify(c));
}, { immediate: true });

watch(draft, () => { saved.value = false; }, { deep: true });

async function save() {
  if (!config.value) return;
  const next = JSON.parse(JSON.stringify(config.value)) as AppConfig;
  await store.saveConfig(next);
  saved.value = true;
  result.value = '设置已保存';
}

async function pickWorkspace() {
  if (!config.value) return;
  const path = await open({ directory: true, multiple: false });
  if (path && typeof path === 'string') config.value.workspace_path = path;
}

/** 思考开关：空串=模型默认(null)，true/false 强制 */
function setThinking(v: string) {
  if (!config.value) return;
  config.value.enable_thinking = v === '' ? null : v === 'true';
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
        <div class="card">
          <div class="card-title">工具工作目录</div>
          <p style="color:var(--text-muted);font-size:12px;margin-top:0;">文件工具与该目录交互，路径越界会被拒绝。留空则使用应用数据目录。</p>
          <div style="display:flex;gap:8px;">
            <input v-model="config.workspace_path" placeholder="留空使用应用数据目录" class="inline-input" />
            <button class="btn" @click="pickWorkspace">选择…</button>
          </div>
        </div>

        <div class="card">
          <div class="card-title">高级生成参数</div>
          <div class="row">
            <div class="field">
              <label>思考过程</label>
              <select :value="config.enable_thinking === null ? '' : String(config.enable_thinking)"
                @change="setThinking(($event.target as HTMLSelectElement).value)">
                <option value="">模型默认</option>
                <option value="true">强制开启</option>
                <option value="false">关闭思考</option>
              </select>
            </div>
            <div class="field">
              <label>温度（创造力，0-2）</label>
              <input v-model.number="config.temperature" type="number" min="0" max="2" step="0.1" placeholder="默认" />
            </div>
            <div class="field">
              <label>最大输出 token</label>
              <input v-model.number="config.max_tokens" type="number" min="0" step="512" placeholder="默认" />
            </div>
          </div>
          <div class="field">
            <label class="toggle-label">
              <input type="checkbox" v-model="config.show_reasoning" />
              在对话中显示 AI 的思考过程（可折叠）
            </label>
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
          <div class="row">
            <div class="field">
              <label>主题</label>
              <select :value="theme.theme" @change="theme.setTheme(($event.target as HTMLSelectElement).value as any)">
                <option v-for="(label, key) in THEME_LABELS" :key="key" :value="key">{{ label }}</option>
              </select>
            </div>
            <div class="field">
              <label>配色</label>
              <select :value="theme.accent" @change="theme.setAccent(($event.target as HTMLSelectElement).value as any)">
                <option v-for="a in accents" :key="a" :value="a">{{ ACCENTS[a] }}</option>
              </select>
            </div>
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
