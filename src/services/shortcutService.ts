import { invoke } from '@tauri-apps/api/core';

/**
 * 全局快捷键配置。组合键字符串存 SQLite settings 表，
 * 注册与触发全在 Rust 侧，前端只负责展示与录制输入。
 */

export interface ShortcutConfig {
  action: string;
  label: string;
  /** 当前组合键，如 "Ctrl+Alt+F"；空串 = 未启用 */
  value: string;
  default: string;
}

export const shortcutService = {
  async getConfig(): Promise<ShortcutConfig[]> {
    return invoke<ShortcutConfig[]>('get_shortcut_config');
  },
  /** 设置组合键（空串 = 停用）。冲突或格式非法时 reject，文案可直接展示。 */
  async set(action: string, value: string): Promise<void> {
    await invoke('set_shortcut', { action, value });
  },
};

/** 存储格式 → 展示格式："Ctrl+Alt+F" → "Ctrl + Alt + F" */
export function formatCombo(combo: string): string {
  return combo.split('+').join(' + ');
}
