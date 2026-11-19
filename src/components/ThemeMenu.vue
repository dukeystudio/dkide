<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useThemeStore, ACCENTS, THEME_LABELS, type Accent } from '../themeStore';

const theme = useThemeStore();
const open = ref(false);

const accents = Object.keys(ACCENTS) as Accent[];

function themeIcon() {
  return theme.theme === 'light' ? '☀️' : theme.theme === 'dark' ? '🌙' : '🖥';
}

function toggle() { open.value = !open.value; }
function close() { open.value = false; }

// 点击外部关闭
function onDoc(e: MouseEvent) {
  if (!(e.target as HTMLElement).closest('.theme-menu')) close();
}
onMounted(() => document.addEventListener('click', onDoc));
onUnmounted(() => document.removeEventListener('click', onDoc));
</script>

<template>
  <div class="theme-menu">
    <button class="theme-toggle" :title="`主题：${THEME_LABELS[theme.theme]}`" @click.stop="toggle">
      <span>{{ themeIcon() }}</span>
    </button>
    <div v-if="open" class="theme-popover" @click.stop>
      <div class="tp-row">
        <span class="tp-label">外观</span>
        <div class="theme-options">
          <button v-for="(label, key) in THEME_LABELS" :key="key"
            class="theme-opt" :class="{ on: theme.theme === key }"
            @click="theme.setTheme(key)">{{ label }}</button>
        </div>
      </div>
      <div class="tp-row">
        <span class="tp-label">配色</span>
        <div class="theme-options">
          <button v-for="a in accents" :key="a"
            class="color-dot" :class="[`dot-${a}`, { on: theme.accent === a }]"
            :title="ACCENTS[a]" @click="theme.setAccent(a)"></button>
        </div>
      </div>
    </div>
  </div>
</template>
