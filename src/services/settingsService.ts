import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';

/**
 * 设置以 key-value 存于 SQLite settings 表。
 * 任何窗口修改设置后广播 settings://changed，其他窗口监听并刷新。
 */
export const SETTINGS_CHANGED_EVENT = 'settings://changed';

/** 主面板被要求打开某个页签时后端推给它（payload: { tab: 'settings' | 'stats' }）。 */
export const PANEL_OPENED_EVENT = 'panel://open';

export const settingsService = {
  async get(key: string): Promise<string | null> {
    return invoke<string | null>('get_setting', { key });
  },
  async set(key: string, value: string): Promise<void> {
    await invoke('set_setting', { key, value });
    await emit(SETTINGS_CHANGED_EVENT, { key, value });
  },
  async openWindow(): Promise<void> {
    await invoke('open_settings');
  },
};
