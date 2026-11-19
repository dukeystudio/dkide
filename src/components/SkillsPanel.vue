<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useSkillsStore } from '../skillsStore';
import { api } from '../api';
import type { MarketSkill } from '../types';

const skills = useSkillsStore();
const url = ref('');
const market = ref<MarketSkill[]>([]);
const marketLoading = ref(false);

onMounted(async () => {
  await skills.load();
  await loadMarket();
});

async function installUrl() {
  await skills.installFromUrl(url.value);
  url.value = '';
}

async function loadMarket() {
  marketLoading.value = true;
  try { market.value = await api.listMarketplace(); } catch { market.value = []; }
  finally { marketLoading.value = false; }
}

async function installFromMarket(s: MarketSkill) {
  await skills.installFromUrl(s.url);
}
</script>

<template>
  <div class="panel-page">
    <div class="panel-inner">
      <h2>技能管理</h2>
      <p style="color:var(--text-muted);margin-top:0;">
        技能是带 <code>SKILL.md</code> 的文件夹，模型可按需自动加载。安装后对话时会自动注入技能清单。
      </p>

      <div style="display:flex;gap:10px;flex-wrap:wrap;margin-bottom:12px;">
        <button class="btn btn-primary" @click="skills.installFromDir()">从文件夹安装</button>
        <button class="btn btn-primary" @click="skills.installFromZip()">从 ZIP 安装</button>
        <button class="btn" @click="skills.openDir()">打开技能目录</button>
        <button class="btn" @click="skills.load()">刷新</button>
      </div>
      <div class="card" style="margin-bottom:18px;">
        <div class="card-title">在线安装（ZIP URL）</div>
        <div style="display:flex;gap:8px;">
          <input v-model="url" class="inline-input" placeholder="https://example.com/my-skill.zip" @keydown.enter="installUrl" />
          <button class="btn btn-primary" @click="installUrl">安装</button>
        </div>
        <div class="skill-meta" style="margin-top:8px;">ZIP 内需有顶层目录包含 SKILL.md。</div>
      </div>

      <div v-if="skills.message" class="toast">{{ skills.message }}</div>

      <div v-if="!skills.skills.length" style="padding:28px;text-align:center;color:var(--text-faint);border:1px dashed var(--border-strong);border-radius:10px;margin-bottom:24px;">
        还没有安装技能。可从文件夹 / ZIP 安装，或从下方技能市场一键安装。
      </div>

      <div v-for="s in skills.skills" :key="s.dir_name" class="skill-card">
        <div style="display:flex;justify-content:space-between;align-items:flex-start;">
          <div>
            <h4>🧩 {{ s.meta.name }} <span class="skill-meta">v{{ s.meta.version || '—' }}</span></h4>
            <div class="desc">{{ s.meta.description || '暂无描述' }}</div>
            <div>
              <span v-for="t in s.meta.allowed_tools" :key="t" class="tag">{{ t }}</span>
              <span class="skill-meta">目录: {{ s.dir_name }}</span>
            </div>
          </div>
          <button class="btn btn-danger btn-sm" @click="skills.uninstall(s.dir_name)">卸载</button>
        </div>
      </div>

      <div style="margin-top:28px;" v-if="market.length">
        <h3>技能市场</h3>
        <p style="color:var(--text-muted);font-size:13px;">一键安装官方与社区技能。</p>
        <div v-for="s in market" :key="s.name" class="skill-card">
          <div style="display:flex;justify-content:space-between;align-items:center;">
            <div>
              <h4>{{ s.name }} <span class="skill-meta">by {{ s.author }}</span></h4>
              <div class="desc" style="margin:0;">{{ s.description }}</div>
            </div>
            <button class="btn btn-sm btn-primary" @click="installFromMarket(s)">安装</button>
          </div>
        </div>
      </div>
      <div v-else-if="marketLoading" class="skill-meta">加载技能市场…</div>
    </div>
  </div>
</template>
