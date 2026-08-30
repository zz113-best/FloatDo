import { create } from 'zustand';
import type { ThemeMode } from '../types';
import { settingsService } from '../services/settingsService';
import { petService, type PetPhotoConfig } from '../services/petService';

interface SettingsState {
  theme: ThemeMode;
  autoExpand: boolean;
  /** 悬浮窗卡片不透明度，0.2 ~ 1 */
  opacity: number;
  /** 是否显示桌宠 */
  petEnabled: boolean;
  /** 照片桌宠配置（null = 尚未加载） */
  petPhoto: PetPhotoConfig | null;
  /** 专注模式：一轮专注时长（分钟） */
  focusWorkMinutes: number;
  /** 专注模式：专注后的休息时长（分钟） */
  focusBreakMinutes: number;
  /** 到期前提前提醒的分钟数 */
  reminderLeadMinutes: number;
  loaded: boolean;
  load: (retry?: number) => Promise<void>;
  refreshPetPhoto: () => Promise<void>;
  setTheme: (theme: ThemeMode) => Promise<void>;
  setAutoExpand: (value: boolean) => Promise<void>;
  setOpacity: (value: number) => Promise<void>;
  setPetEnabled: (value: boolean) => Promise<void>;
  setFocusWorkMinutes: (value: number) => Promise<void>;
  setFocusBreakMinutes: (value: number) => Promise<void>;
  setReminderLeadMinutes: (value: number) => Promise<void>;
}

/** 根据主题模式给 <html> 挂 dark class（配合 Tailwind class 策略）。 */
export function applyTheme(theme: ThemeMode): void {
  const root = document.documentElement;
  if (theme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    root.classList.toggle('dark', prefersDark);
  } else {
    root.classList.toggle('dark', theme === 'dark');
  }
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  theme: 'system',
  autoExpand: true,
  opacity: 0.96,
  petEnabled: true,
  petPhoto: null,
  focusWorkMinutes: 25,
  focusBreakMinutes: 5,
  reminderLeadMinutes: 10,
  loaded: false,

  async load(retry = 0) {
    try {
      const [theme, autoExpand, opacity, petEnabled, focusWork, focusBreak, reminderLead] =
        await Promise.all([
          settingsService.get('theme'),
          settingsService.get('autoExpand'),
          settingsService.get('opacity'),
          settingsService.get('petEnabled'),
          settingsService.get('focusWorkMinutes'),
          settingsService.get('focusBreakMinutes'),
          settingsService.get('reminderLeadMinutes'),
        ]);
      set({
        theme: (theme as ThemeMode) ?? 'system',
        autoExpand: autoExpand !== null ? autoExpand === 'true' : true,
        opacity: opacity !== null ? Number(opacity) : 0.96,
        petEnabled: petEnabled !== null ? petEnabled !== 'false' : true,
        focusWorkMinutes: clampFocusMinutes(focusWork, 25),
        focusBreakMinutes: clampFocusMinutes(focusBreak, 5),
        reminderLeadMinutes: clampNumber(Number(reminderLead), 1, 1440, 10),
        loaded: true,
      });
      applyTheme(get().theme);
    } catch {
      // 启动瞬间后端可能尚未就绪，稍候重试
      if (retry < 5) {
        setTimeout(() => void get().load(retry + 1), 600 * (retry + 1));
      }
    }
  },

  /** 重新拉取照片桌宠配置（初次加载 + pet://photo-changed 时都会调用）。 */
  async refreshPetPhoto() {
    try {
      set({ petPhoto: await petService.getPhoto() });
    } catch {
      // 后端尚未就绪时保持现状，等下一次事件再刷新
    }
  },

  async setTheme(theme) {
    set({ theme });
    applyTheme(theme);
    await settingsService.set('theme', theme).catch(() => get().load());
  },

  async setAutoExpand(value) {
    set({ autoExpand: value });
    await settingsService.set('autoExpand', String(value)).catch(() => get().load());
  },

  async setOpacity(value) {
    set({ opacity: value });
    await settingsService.set('opacity', String(value)).catch(() => get().load());
  },

  /** 显示/隐藏桌宠：窗口显隐与持久化都在 Rust 端 set_pet_visible 完成。 */
  async setPetEnabled(value) {
    set({ petEnabled: value });
    await petService.setVisible(value).catch(() => get().load());
  },

  async setFocusWorkMinutes(value) {
    const v = clampNumber(value, 1, 180, 25);
    set({ focusWorkMinutes: v });
    await settingsService.set('focusWorkMinutes', String(v)).catch(() => get().load());
  },

  async setFocusBreakMinutes(value) {
    const v = clampNumber(value, 1, 180, 5);
    set({ focusBreakMinutes: v });
    await settingsService.set('focusBreakMinutes', String(v)).catch(() => get().load());
  },

  async setReminderLeadMinutes(value) {
    const v = clampNumber(value, 1, 1440, 10);
    set({ reminderLeadMinutes: v });
    await settingsService.set('reminderLeadMinutes', String(v)).catch(() => get().load());
  },
}));

/** 解析存储里的分钟数，非法或越界时回落默认值（与后端 1~180 的校验一致）。 */
function clampFocusMinutes(raw: string | null, fallback: number): number {
  if (raw === null) return fallback;
  const n = Number(raw);
  return clampNumber(n, 1, 180, fallback);
}

function clampNumber(value: number, min: number, max: number, fallback: number): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}
