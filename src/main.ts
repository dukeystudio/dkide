import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import './style.css';
import { useThemeStore } from './themeStore';

const pinia = createPinia();

// 在挂载前立即应用已保存的主题/配色，避免首屏按默认浅色闪烁
useThemeStore(pinia);

createApp(App).use(pinia).mount('#app');
