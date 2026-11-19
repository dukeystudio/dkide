import { defineStore } from 'pinia';
import { ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from './api';
import type { Skill } from './types';

export const useSkillsStore = defineStore('skills', () => {
  const skills = ref<Skill[]>([]);
  const loading = ref(false);
  const message = ref('');

  async function load() {
    loading.value = true;
    try {
      skills.value = await api.listSkills();
    } finally {
      loading.value = false;
    }
  }

  async function installFromDir() {
    const path = await open({ directory: true, multiple: false });
    if (!path || typeof path !== 'string') return;
    try {
      await api.installSkillFromDir(path);
      message.value = '技能安装成功';
      await load();
    } catch (e: any) {
      message.value = `安装失败: ${e}`;
    }
  }

  async function installFromZip() {
    const path = await open({
      multiple: false,
      filters: [{ name: 'Zip', extensions: ['zip'] }],
    });
    if (!path || typeof path !== 'string') return;
    try {
      await api.installSkillFromZip(path);
      message.value = '技能安装成功';
      await load();
    } catch (e: any) {
      message.value = `安装失败: ${e}`;
    }
  }

  async function installFromUrl(url: string) {
    if (!url.trim()) return;
    message.value = '正在下载安装…';
    try {
      const result = await api.installSkillFromUrl(url.trim());
      message.value = `安装成功，共 ${result.length} 个技能`;
      await load();
    } catch (e: any) {
      const text = typeof e === 'string' ? e : e?.message ?? String(e);
      message.value = `安装失败: ${text}`;
    }
  }

  async function uninstall(dirName: string) {
    try {
      await api.uninstallSkill(dirName);
      message.value = '技能已卸载';
      await load();
    } catch (e: any) {
      message.value = `卸载失败: ${e}`;
    }
  }

  async function openDir() {
    await api.openSkillsDir();
  }

  return {
    skills,
    loading,
    message,
    load,
    installFromDir,
    installFromZip,
    installFromUrl,
    uninstall,
    openDir,
  };
});
