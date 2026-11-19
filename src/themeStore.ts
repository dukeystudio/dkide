import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

export type ThemeMode = 'system' | 'light' | 'dark';
export type Accent = 'indigo' | 'teal' | 'rose' | 'orange';

export const ACCENTS: Record<Accent, string> = {
  indigo: '靛蓝',
  teal: '青绿',
  rose: '玫红',
  orange: '暖橙',
};

export const THEME_LABELS: Record<ThemeMode, string> = {
  system: '跟随系统',
  light: '浅色',
  dark: '深色',
};

const KEY = 'dkide-theme';

function systemDark() {
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
}

function apply(theme: ThemeMode, accent: Accent) {
  const root = document.documentElement;
  const resolved = theme === 'system' ? (systemDark() ? 'dark' : 'light') : theme;
  root.setAttribute('data-theme', resolved);
  root.setAttribute('data-accent', accent);
}

export const useThemeStore = defineStore('theme', () => {
  const saved = (() => {
    try {
      const raw = localStorage.getItem(KEY);
      if (raw) return JSON.parse(raw) as Partial<{ theme: ThemeMode; accent: Accent }>;
    } catch { /* ignore */ }
    return {};
  })();

  const theme = ref<ThemeMode>(saved.theme ?? 'system');
  const accent = ref<Accent>(saved.accent ?? 'indigo');

  watch([theme, accent], ([t, a]) => {
    apply(t, a);
    try { localStorage.setItem(KEY, JSON.stringify({ theme: t, accent: a })); } catch { /* ignore */ }
  }, { immediate: true });

  function setTheme(next: ThemeMode) { theme.value = next; }
  function setAccent(next: Accent) { accent.value = next; }
  function cycleTheme() {
    // 浅色 → 深色 → 跟随系统 → 浅色
    theme.value = theme.value === 'light' ? 'dark' : theme.value === 'dark' ? 'system' : 'light';
  }

  // 跟随系统模式下，系统深浅变化时实时应用
  if (typeof window !== 'undefined' && window.matchMedia) {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
      if (theme.value === 'system') apply(theme.value, accent.value);
    });
  }

  return { theme, accent, setTheme, setAccent, cycleTheme };
});
