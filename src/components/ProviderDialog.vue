<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { useChatStore } from '../store';
import { api } from '../api';
import type { Provider, AppConfig } from '../types';

const store = useChatStore();
const open = computed(() => store.providerDialogOpen);
const focusAdd = computed(() => store.focusAddProvider);

// 本地草稿，避免未点保存就改到内存配置导致前后端分叉
const draft = ref<AppConfig | null>(null);
const config = computed(() => draft.value);
const saved = ref(true);

watch(open, (o) => {
  if (o) {
    // 每次打开都从当前配置重建草稿，保证是最新值
    const c = store.config;
    if (c) draft.value = JSON.parse(JSON.stringify(c));
    if (focusAdd.value) {
      store.focusAddProvider = false;
      nextTick(scrollToAdd);
    }
  }
});
watch(() => store.config, (c) => {
  if (c && !draft.value) draft.value = JSON.parse(JSON.stringify(c));
}, { immediate: true });
watch(draft, () => { saved.value = false; }, { deep: true });

/** 常用服务商预设：选中即填好 base_url 与模型，用户再各自申请 key 填入 */
const presets = [
  { name: 'Agnes（国内）', url: 'https://api.agnes-ai.cn/v1', model: 'agnes-3.0-flash', apply: 'https://platform.agnes-ai.cn' },
  { name: 'OpenAI', url: 'https://api.openai.com/v1', model: 'gpt-4o-mini', apply: 'https://platform.openai.com/api-keys' },
  { name: 'DeepSeek', url: 'https://api.deepseek.com/v1', model: 'deepseek-chat', apply: 'https://platform.deepseek.com/api_keys' },
  { name: 'Moonshot', url: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k', apply: 'https://platform.moonshot.cn' },
  { name: '智谱', url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4', apply: 'https://open.bigmodel.cn' },
];

function applyLink(p: Provider): string | null {
  const host = (u: string) => { try { return new URL(u).host; } catch { return u; } };
  return presets.find((x) => host(x.url) === host(p.base_url))?.apply ?? null;
}

function addProvider(preset?: (typeof presets)[0]) {
  if (!config.value) return;
  config.value.providers.push({
    id: crypto.randomUUID(),
    name: preset?.name ?? '新中转站',
    base_url: preset?.url ?? 'https://api.openai.com/v1',
    api_key: '',
    model: preset?.model ?? 'gpt-4o-mini',
  });
  nextTick(scrollToAdd);
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
  close();
}

const testing = ref(false);
const testProviderId = ref('');
const testResults = ref<Record<string, { ok: boolean; text: string }>>({});
function isTesting(id: string) { return testing.value && testProviderId.value === id; }
async function test(p: Provider) {
  if (isTesting(p.id)) return;
  const snap: Provider = JSON.parse(JSON.stringify(p));
  testProviderId.value = p.id;
  testing.value = true;
  testResults.value[p.id] = { ok: false, text: '测试中…' };
  try {
    const ok = await api.testProvider(snap);
    testResults.value[p.id] = { ok: true, text: `✓ ${ok}` };
  } catch (e: any) {
    const msg = typeof e === 'string' ? e : e?.message ?? String(e);
    testResults.value[p.id] = { ok: false, text: `✗ ${msg}` };
  } finally {
    if (testProviderId.value === p.id) {
      testProviderId.value = '';
      testing.value = false;
    }
  }
}

function close() {
  store.providerDialogOpen = false;
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && open.value) close();
}
onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));

const addGroupEl = ref<HTMLElement | null>(null);
function scrollToAdd() {
  addGroupEl.value?.scrollIntoView({ behavior: 'smooth', block: 'center' });
}
</script>

<template>
  <!-- 点空白不关闭：mask 不加 @click.self，只有关闭按钮 / 保存 / 取消 / ESC -->
  <div v-if="open" class="modal-mask provider-mask" role="dialog" aria-modal="true" aria-label="服务商配置">
    <div class="modal provider-modal">
      <div class="provider-head">
        <h3>模型服务商</h3>
        <button class="modal-close" title="关闭" @click="close">✕</button>
      </div>
      <p>配置 OpenAI 兼容中转站，可添加多个并切换。API Key 只存在你本机钥匙串。</p>

      <template v-if="config">
        <div class="provider-list">
          <div v-for="p in config.providers" :key="p.id" class="provider-item" :class="{ active: config.active_provider_id === p.id }">
            <div class="provider-head-row">
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
            <div class="field">
              <label>API Key（存入系统钥匙串）</label>
              <input v-model="p.api_key" type="password" placeholder="sk-... / cpk-..." />
            </div>
            <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
              <button class="btn btn-sm" :disabled="testing && testProviderId !== p.id" @click="test(p)">
                {{ testing && testProviderId === p.id ? '测试中…' : '测试连接' }}
              </button>
              <span v-if="testResults[p.id]" class="test-result" :class="{ ok: testResults[p.id].ok }">{{ testResults[p.id].text }}</span>
              <a v-if="applyLink(p)" :href="applyLink(p) as string" target="_blank" class="apply-link" rel="noopener">没有 key？去申请 →</a>
            </div>
          </div>
        </div>

        <div ref="addGroupEl" class="preset-group">
          <div class="preset-label">添加服务商（点选自动填好 Base URL 与模型，再填 key）</div>
          <div style="display:flex;gap:8px;flex-wrap:wrap;">
            <button v-for="p in presets" :key="p.name" class="btn btn-sm" @click="addProvider(p)">+ {{ p.name }}</button>
            <button class="btn btn-sm" @click="addProvider()">+ 自定义</button>
          </div>
        </div>

        <div class="modal-actions">
          <span v-if="!saved" class="tag" style="background:var(--warn-bg);color:var(--warn-text);margin-right:auto;">● 有未保存的更改</span>
          <button class="btn" @click="close">取消</button>
          <button class="btn btn-primary" @click="save">保存</button>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.provider-mask { z-index: 150; }
.provider-modal { width: 560px; max-width: 94vw; max-height: 86vh; overflow: auto; }
.provider-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px; }
.provider-head h3 { margin: 0; }
.modal-close { background: none; border: none; font-size: 16px; cursor: pointer; color: var(--text-muted); }
.provider-list { display: flex; flex-direction: column; gap: 12px; margin: 14px 0; }
.provider-head-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.preset-group { margin-top: 16px; padding-top: 14px; border-top: 1px solid var(--border); }
</style>
