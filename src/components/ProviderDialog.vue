<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { useChatStore } from '../store';
import { api } from '../api';
import type { AppConfig, Provider } from '../types';

const store = useChatStore();
const open = computed(() => store.providerDialogOpen);
const focusAdd = computed(() => store.focusAddProvider);

const draft = ref<AppConfig | null>(null);
const config = computed(() => draft.value);
const saved = ref(true);
const formError = ref('');
const apiKeyVisible = ref(false);
const presetChoice = ref('');
const testing = ref(false);
const testProviderId = ref('');
const testResults = ref<Record<string, { ok: boolean; text: string }>>({});
const formEl = ref<HTMLElement | null>(null);

const presets = [
  { name: 'Agnes（国内）', url: 'https://api.agnes-ai.cn/v1', model: 'agnes-3.0-flash', apply: 'https://platform.agnes-ai.cn' },
  { name: 'OpenAI', url: 'https://api.openai.com/v1', model: 'gpt-4o-mini', apply: 'https://platform.openai.com/api-keys' },
  { name: 'DeepSeek', url: 'https://api.deepseek.com/v1', model: 'deepseek-chat', apply: 'https://platform.deepseek.com/api_keys' },
  { name: 'Moonshot', url: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k', apply: 'https://platform.moonshot.cn' },
  { name: '智谱', url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4', apply: 'https://open.bigmodel.cn' },
];

const selectedProvider = computed<Provider | null>(() => {
  const current = config.value;
  if (!current) return null;
  return current.providers.find((p) => p.id === current.active_provider_id) ?? current.providers[0] ?? null;
});

watch(open, async (isOpen) => {
  if (!isOpen) return;
  const current = store.config;
  if (current) draft.value = JSON.parse(JSON.stringify(current)) as AppConfig;
  const focusProviderId = store.providerDialogFocusId;
  if (focusProviderId && draft.value?.providers.some((provider) => provider.id === focusProviderId)) {
    draft.value.active_provider_id = focusProviderId;
  }
  store.providerDialogFocusId = null;
  formError.value = '';
  apiKeyVisible.value = false;
  presetChoice.value = '';
  saved.value = true;
  if (focusAdd.value || focusProviderId) {
    store.focusAddProvider = false;
    await nextTick();
    scrollToForm();
  }
});
watch(() => store.config, (current) => {
  if (current && !draft.value) draft.value = JSON.parse(JSON.stringify(current)) as AppConfig;
}, { immediate: true });
watch(draft, () => { saved.value = false; }, { deep: true, flush: 'sync' });

function applyLink(provider: Provider): string | null {
  const host = (url: string) => { try { return new URL(url).host; } catch { return url; } };
  return presets.find((preset) => host(preset.url) === host(provider.base_url))?.apply ?? null;
}

function addProvider() {
  if (!config.value) return;
  const preset = presets.find((p) => p.name === presetChoice.value);
  const provider: Provider = {
    id: crypto.randomUUID(),
    name: preset?.name ?? 'Custom',
    base_url: preset?.url ?? 'https://api.openai.com/v1',
    api_key: '',
    model: preset?.model ?? 'gpt-4o-mini',
    reasoning_effort: null,
    tool_calling: true,
    image_input: true,
    reasoning: true,
    input_tokens: null,
    output_tokens: null,
  };
  config.value.providers.push(provider);
  config.value.active_provider_id = provider.id;
  presetChoice.value = '';
  formError.value = '';
  nextTick(scrollToForm);
}

function removeProvider() {
  if (!config.value || !selectedProvider.value) return;
  if (config.value.providers.length <= 1) {
    formError.value = '至少保留一个服务商';
    return;
  }
  const removedId = selectedProvider.value.id;
  config.value.providers = config.value.providers.filter((p) => p.id !== removedId);
  config.value.active_provider_id = config.value.providers[0].id;
  formError.value = '';
}

function setReasoningEffort(event: Event) {
  if (!selectedProvider.value) return;
  const value = (event.target as HTMLInputElement).value.trim();
  selectedProvider.value.reasoning_effort = value || null;
}

function setTokenValue(kind: 'input_tokens' | 'output_tokens', event: Event) {
  if (!selectedProvider.value) return;
  const raw = (event.target as HTMLInputElement).value.trim();
  const value = Number(raw);
  selectedProvider.value[kind] = raw && Number.isFinite(value) && value > 0 ? Math.round(value) : null;
}

function chooseTokenValue(kind: 'input_tokens' | 'output_tokens', value: number | null) {
  if (selectedProvider.value) selectedProvider.value[kind] = value;
}

async function save() {
  if (!config.value || !selectedProvider.value) return;
  const provider = selectedProvider.value;
  if (!provider.name.trim()) { formError.value = '请先填写显示名称'; return; }
  if (!provider.base_url.trim()) { formError.value = '请先填写 Base URL'; return; }
  if (!provider.model.trim()) { formError.value = '请先填写模型名称'; return; }
  if (!provider.api_key.trim()) { formError.value = '请先填写 API Key'; return; }
  try {
    const next = JSON.parse(JSON.stringify(config.value)) as AppConfig;
    await store.saveConfig(next);
    saved.value = true;
    close();
  } catch (error) {
    const message = typeof error === 'string' ? error : (error as Error)?.message ?? String(error);
    formError.value = `保存失败：${message}`;
  }
}

const isTestingSelected = computed(() => testing.value && testProviderId.value === selectedProvider.value?.id);

async function test() {
  const provider = selectedProvider.value;
  if (!provider || isTestingSelected.value) return;
  const snapshot: Provider = JSON.parse(JSON.stringify(provider));
  testProviderId.value = provider.id;
  testing.value = true;
  testResults.value[provider.id] = { ok: false, text: '测试中…' };
  formError.value = '';
  try {
    const message = await api.testProvider(snapshot);
    testResults.value[provider.id] = { ok: true, text: `✓ ${message}` };
  } catch (error) {
    const message = typeof error === 'string' ? error : (error as Error)?.message ?? String(error);
    testResults.value[provider.id] = { ok: false, text: `✗ ${message}` };
  } finally {
    if (testProviderId.value === provider.id) {
      testProviderId.value = '';
      testing.value = false;
    }
  }
}

function close() {
  store.providerDialogOpen = false;
}
function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && open.value) close();
}
function scrollToForm() {
  formEl.value?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}
onMounted(() => document.addEventListener('keydown', onKeydown));
onUnmounted(() => document.removeEventListener('keydown', onKeydown));
</script>

<template>
  <div v-if="open" class="modal-mask provider-mask" role="dialog" aria-modal="true" aria-label="服务商配置">
    <div class="modal provider-modal">
      <div class="provider-head">
        <h3>模型服务商</h3>
        <button class="modal-close" title="关闭" @click="close">✕</button>
      </div>
      <p class="provider-intro">配置 OpenAI 兼容服务商。API Key 只保存在本机钥匙串。</p>

      <template v-if="config && selectedProvider">
        <div ref="formEl" class="provider-form">
          <div class="provider-select-row">
            <div class="field provider-grow">
              <label>当前供应商</label>
              <select v-model="config.active_provider_id">
                <option v-for="provider in config.providers" :key="provider.id" :value="provider.id">{{ provider.name }}</option>
              </select>
            </div>
            <button class="btn provider-action" :disabled="config.providers.length <= 1" @click="removeProvider">删除</button>
          </div>

          <div class="field">
            <label>显示名称</label>
            <input v-model="selectedProvider.name" placeholder="例如：我的 DeepSeek" />
          </div>

          <div class="provider-select-row">
            <div class="field provider-grow">
              <label>新增服务商（选择预设或自定义）</label>
              <select v-model="presetChoice">
                <option value="">自定义（手动填写）</option>
                <option v-for="preset in presets" :key="preset.name" :value="preset.name">{{ preset.name }}</option>
              </select>
            </div>
            <button class="btn btn-primary provider-action" @click="addProvider">添加</button>
          </div>
          <p class="provider-note">选择预设会自动填入 Base URL 与模型；选择自定义请手动填写。</p>

          <div class="field">
            <label>Base URL</label>
            <input v-model="selectedProvider.base_url" placeholder="https://api.example.com/v1" />
          </div>

          <div class="field">
            <label>API Key</label>
            <div class="key-field">
              <input v-model="selectedProvider.api_key" :type="apiKeyVisible ? 'text' : 'password'" placeholder="sk-... / cpk-..." />
              <button class="key-toggle" type="button" @click="apiKeyVisible = !apiKeyVisible">{{ apiKeyVisible ? '隐藏' : '显示' }}</button>
            </div>
          </div>

          <div class="field">
            <label>模型名称</label>
            <input v-model="selectedProvider.model" placeholder="gpt-4o-mini" />
          </div>

          <div class="field">
            <label>思考强度 <span class="field-help" title="例如 low / medium / high / xhigh，留空使用模型默认值">(i)</span></label>
            <input :value="selectedProvider.reasoning_effort ?? ''" placeholder="例如 high，留空使用模型默认值" @input="setReasoningEffort" />
          </div>

          <div class="provider-section">
            <h4>高级设置</h4>
            <div class="capability-grid">
              <label class="capability-item">
                <input v-model="selectedProvider.tool_calling" type="checkbox" />
                <span>工具调用</span>
              </label>
              <label class="capability-item">
                <input v-model="selectedProvider.image_input" type="checkbox" />
                <span>图片输入</span>
              </label>
              <label class="capability-item">
                <input v-model="selectedProvider.reasoning" type="checkbox" />
                <span>推理</span>
              </label>
            </div>
          </div>

          <div class="provider-section">
            <label class="provider-label">输入</label>
            <input
              class="token-input"
              inputmode="numeric"
              :value="selectedProvider.input_tokens ?? ''"
              placeholder="使用供应商默认值"
              @input="setTokenValue('input_tokens', $event)"
            />
            <div class="token-chips">
              <button
                v-for="value in [32768, 65536, 131072, 262144]"
                :key="value"
                class="token-chip"
                :class="{ active: selectedProvider.input_tokens === value }"
                @click="chooseTokenValue('input_tokens', value)"
              >{{ value / 1024 }}K</button>
              <button class="token-chip" :class="{ active: selectedProvider.input_tokens === null }" @click="chooseTokenValue('input_tokens', null)">默认</button>
            </div>
          </div>

          <div class="provider-section">
            <label class="provider-label">输出</label>
            <input
              class="token-input"
              inputmode="numeric"
              :value="selectedProvider.output_tokens ?? ''"
              placeholder="使用供应商默认值"
              @input="setTokenValue('output_tokens', $event)"
            />
            <div class="token-chips">
              <button
                v-for="value in [8192, 16384, 32768, 65536]"
                :key="value"
                class="token-chip"
                :class="{ active: selectedProvider.output_tokens === value }"
                @click="chooseTokenValue('output_tokens', value)"
              >{{ value / 1024 }}K</button>
              <button class="token-chip" :class="{ active: selectedProvider.output_tokens === null }" @click="chooseTokenValue('output_tokens', null)">默认</button>
            </div>
          </div>

          <div class="provider-test-row">
            <button class="btn btn-sm" :disabled="isTestingSelected" @click="test">{{ isTestingSelected ? '测试中…' : '测试连接' }}</button>
            <span v-if="testResults[selectedProvider.id]" class="test-result" :class="{ ok: testResults[selectedProvider.id].ok }">{{ testResults[selectedProvider.id].text }}</span>
            <a v-if="applyLink(selectedProvider)" :href="applyLink(selectedProvider) as string" target="_blank" rel="noopener" class="apply-link">没有 key？去申请 →</a>
          </div>
          <p v-if="formError" class="provider-error">{{ formError }}</p>
          <p class="provider-note">测试连接仅验证地址、Key 与模型；思考强度是否生效以服务商实际支持为准。</p>
        </div>

        <div class="modal-actions">
          <span v-if="!saved" class="tag provider-unsaved">● 有未保存的更改</span>
          <button class="btn" @click="close">取消</button>
          <button class="btn btn-primary" @click="save">保存</button>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.provider-mask { z-index: 150; }
.provider-modal { width: 520px; max-width: 94vw; max-height: 88vh; overflow: auto; }
.provider-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px; }
.provider-head h3 { margin: 0; }
.provider-intro { color: var(--text-muted); line-height: 1.6; margin: 0 0 16px; }
.provider-form { display: flex; flex-direction: column; gap: 14px; }
.provider-modal .btn { font-weight: 400; white-space: nowrap; }
.modal-close { color: var(--text-muted); }
.provider-select-row { display: flex; align-items: flex-end; gap: 10px; }
.provider-grow { flex: 1; min-width: 0; margin-bottom: 0; }
.provider-action { margin-bottom: 0; height: 38px; }
.provider-field-label,
.provider-label { display: block; font-size: 12px; color: var(--text-muted); margin-bottom: 6px; white-space: nowrap; }
.field label { white-space: nowrap; }
.field-help { color: var(--text-faint); font-size: 11px; }
.key-field { display: flex; gap: 8px; }
.key-field input { flex: 1; min-width: 0; }
.key-toggle { min-width: 44px; padding: 0 8px; border: 1px solid var(--border-strong); border-radius: 7px; background: var(--bg-subtle); color: var(--text-muted); font-size: 12px; white-space: nowrap; }
.key-toggle:hover { background: var(--bg-hover); }
.provider-section { padding-top: 12px; border-top: 1px solid var(--border); }
.provider-section h4 { margin: 0 0 10px; font-size: 13px; }
.capability-grid { display: flex; gap: 20px; flex-wrap: nowrap; }
.capability-item { display: flex; align-items: center; gap: 7px; color: var(--text); font-size: 13px; white-space: nowrap; cursor: pointer; }
.capability-item input { accent-color: var(--primary); }
.token-input { width: 100%; }
.token-chips { display: flex; gap: 8px; margin-top: 8px; flex-wrap: nowrap; }
.token-chip { padding: 4px 10px; border-radius: 6px; border: 1px solid var(--border); color: var(--text-muted); font-size: 12px; background: var(--bg-subtle); white-space: nowrap; }
.token-chip:hover { background: var(--bg-hover); }
.token-chip.active { border-color: var(--primary); color: var(--primary); background: var(--primary-soft); }
.provider-test-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.provider-error { margin: 0; color: var(--danger-text); font-size: 12px; }
.provider-note { margin: 0; color: var(--text-faint); font-size: 11px; line-height: 1.6; }
.provider-unsaved { background: var(--warn-bg); color: var(--warn-text); margin-right: auto; }
</style>
