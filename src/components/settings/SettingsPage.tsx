import { useEffect, useState } from 'react';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { useSettingsStore } from '../../stores/settingsStore';
import { shortcutService, type ShortcutConfig } from '../../services/shortcutService';
import { aiService, type AiConfig } from '../../services/aiService';
import { ShortcutInput } from './ShortcutInput';
import type { ThemeMode } from '../../types';

const THEME_OPTIONS: { value: ThemeMode; label: string }[] = [
  { value: 'light', label: '浅色' },
  { value: 'dark', label: '深色' },
  { value: 'system', label: '跟随系统' },
];

/** 设置窗口页面（独立窗口，路由 #/settings）。 */
export function SettingsPage() {
  const {
    theme,
    autoExpand,
    opacity,
    focusWorkMinutes,
    focusBreakMinutes,
    reminderLeadMinutes,
    setTheme,
    setAutoExpand,
    setOpacity,
    setFocusWorkMinutes,
    setFocusBreakMinutes,
    setReminderLeadMinutes,
  } = useSettingsStore();
  const [autostart, setAutostart] = useState(false);
  const [autostartError, setAutostartError] = useState<string | null>(null);
  // 输入框本地草稿，失焦或回车时才提交，避免每敲一位数字就写库
  const [workDraft, setWorkDraft] = useState(String(focusWorkMinutes));
  const [breakDraft, setBreakDraft] = useState(String(focusBreakMinutes));
  const [focusError, setFocusError] = useState<string | null>(null);
  // 全局快捷键配置（后端 settings 表读出，null = 尚未加载）
  const [shortcuts, setShortcuts] = useState<ShortcutConfig[] | null>(null);
  // AI 接口配置
  const [aiConfig, setAiConfig] = useState<AiConfig | null>(null);
  const [aiBaseUrl, setAiBaseUrl] = useState('');
  const [aiApiKey, setAiApiKey] = useState('');
  const [aiModel, setAiModel] = useState('');
  const [aiStatus, setAiStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const [aiBusy, setAiBusy] = useState(false);

  useEffect(() => {
    setWorkDraft(String(focusWorkMinutes));
  }, [focusWorkMinutes]);
  useEffect(() => {
    setBreakDraft(String(focusBreakMinutes));
  }, [focusBreakMinutes]);

  useEffect(() => {
    isEnabled()
      .then(setAutostart)
      .catch(() => setAutostart(false));
    void shortcutService
      .getConfig()
      .then(setShortcuts)
      .catch(() => setShortcuts([]));
    void aiService
      .getConfig()
      .then((c) => {
        setAiConfig(c);
        setAiBaseUrl(c.baseUrl);
        setAiModel(c.model);
      })
      .catch(() => undefined);
  }, []);

  const toggleAutostart = async (value: boolean) => {
    try {
      if (value) await enable();
      else await disable();
      setAutostart(value);
      setAutostartError(null);
    } catch (e) {
      setAutostartError(`开机启动设置失败: ${String(e)}`);
    }
  };

  const commitFocusMinutes = (kind: 'work' | 'break', draft: string) => {
    const n = Number(draft);
    if (!Number.isFinite(n) || n < 1 || n > 180) {
      setFocusError('专注时长需在 1 ~ 180 分钟之间');
      setWorkDraft(String(focusWorkMinutes));
      setBreakDraft(String(focusBreakMinutes));
      return;
    }
    setFocusError(null);
    if (kind === 'work') void setFocusWorkMinutes(n);
    else void setFocusBreakMinutes(n);
  };

  const commitShortcut = async (action: string, value: string) => {
    await shortcutService.set(action, value);
    setShortcuts(await shortcutService.getConfig());
  };

  const saveAiConfig = async () => {
    setAiBusy(true);
    setAiStatus(null);
    try {
      await aiService.setConfig(aiBaseUrl, aiApiKey, aiModel);
      const fresh = await aiService.getConfig();
      setAiConfig(fresh);
      setAiApiKey('');
      setAiStatus({ ok: true, text: '已保存' });
    } catch (e) {
      setAiStatus({ ok: false, text: `保存失败: ${String(e)}` });
    } finally {
      setAiBusy(false);
    }
  };

  const testAiConfig = async () => {
    setAiBusy(true);
    setAiStatus(null);
    try {
      const reply = await aiService.test();
      setAiStatus({ ok: true, text: `连接成功，模型回复：${reply.slice(0, 40)}` });
    } catch (e) {
      setAiStatus({ ok: false, text: String(e) });
    } finally {
      setAiBusy(false);
    }
  };

  return (
    <div className="p-6">
      <div className="mx-auto max-w-4xl space-y-4">
        <h1 className="text-lg font-semibold">设置</h1>

        {/* 双栏：左边观感与行为，右边快捷键与 AI 配置 */}
        <div className="grid grid-cols-2 items-start gap-4">
          <div className="space-y-4">

        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 text-sm font-medium">外观</h2>
          <div className="mb-4 flex items-center justify-between">
            <span className="text-sm">主题</span>
            <div className="flex gap-1">
              {THEME_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => void setTheme(opt.value)}
                  className={`rounded-md px-3 py-1 text-sm transition ${
                    theme === opt.value
                      ? 'bg-blue-500 text-white'
                      : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-600'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-sm">悬浮窗不透明度</span>
            <div className="flex items-center gap-2">
              <input
                type="range"
                min={20}
                max={100}
                value={Math.round(opacity * 100)}
                onChange={(e) => void setOpacity(Number(e.target.value) / 100)}
                className="w-40"
              />
              <span className="w-10 text-right text-xs tabular-nums">
                {Math.round(opacity * 100)}%
              </span>
            </div>
          </div>
        </section>

        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 text-sm font-medium">行为</h2>
          <label className="flex cursor-pointer items-center justify-between">
            <span className="text-sm">鼠标悬停时自动展开任务列表</span>
            <input
              type="checkbox"
              checked={autoExpand}
              onChange={(e) => void setAutoExpand(e.target.checked)}
              className="h-4 w-4 accent-blue-500"
            />
          </label>
          <label className="mt-3 flex cursor-pointer items-center justify-between">
            <span className="text-sm">Windows 启动时自动运行</span>
            <input
              type="checkbox"
              checked={autostart}
              onChange={(e) => void toggleAutostart(e.target.checked)}
              className="h-4 w-4 accent-blue-500"
            />
          </label>
          <div className="mt-3 flex items-center justify-between">
            <span className="text-sm">到期前提前提醒</span>
            <select
              value={reminderLeadMinutes}
              onChange={(e) => void setReminderLeadMinutes(Number(e.target.value))}
              title="逾期提醒（逾期后 2 分钟内）不受此项影响"
              className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none dark:border-white/20"
            >
              {[5, 10, 15, 30, 60].map((m) => (
                <option key={m} value={m}>
                  提前 {m} 分钟
                </option>
              ))}
            </select>
          </div>
          {autostartError && (
            <p className="mt-2 text-xs text-red-500">{autostartError}</p>
          )}
        </section>

        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 text-sm font-medium">专注</h2>
          <div className="flex items-center justify-between">
            <span className="text-sm">专注时长（分钟）</span>
            <input
              type="number"
              min={1}
              max={180}
              value={workDraft}
              onChange={(e) => setWorkDraft(e.target.value)}
              onBlur={() => commitFocusMinutes('work', workDraft)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
              }}
              className="w-20 rounded-md border border-black/10 px-2 py-1 text-sm tabular-nums outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
            />
          </div>
          <div className="mt-3 flex items-center justify-between">
            <span className="text-sm">休息时长（分钟）</span>
            <input
              type="number"
              min={1}
              max={180}
              value={breakDraft}
              onChange={(e) => setBreakDraft(e.target.value)}
              onBlur={() => commitFocusMinutes('break', breakDraft)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
              }}
              className="w-20 rounded-md border border-black/10 px-2 py-1 text-sm tabular-nums outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
            />
          </div>
          <p className="mt-2 text-xs text-zinc-400">
            悬浮窗展开后可开始专注：专注一轮后自动进入休息，桌宠会陪你一起计时。
          </p>
          {focusError && <p className="mt-2 text-xs text-red-500">{focusError}</p>}
        </section>

          </div>

          <div className="space-y-4">
            <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
              <h2 className="mb-3 text-sm font-medium">全局快捷键</h2>
          {(shortcuts ?? []).map((sc, i) => (
            <div key={sc.action} className={i === 0 ? '' : 'mt-3'}>
              <div className="flex items-center justify-between">
                <span className="text-sm">{sc.label}</span>
                <ShortcutInput value={sc.value} onCommit={(v) => commitShortcut(sc.action, v)} />
              </div>
            </div>
          ))}
          <p className="mt-3 text-xs leading-relaxed text-zinc-400">
            在任何应用前台都能触发。点击右侧按钮后直接按下组合键即可录制；
            Backspace 清除表示停用。提示「已被占用」就换一个组合。
          </p>
        </section>

        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 text-sm font-medium">AI 助手</h2>
          <div className="space-y-3">
            <label className="block">
              <span className="text-sm">接口地址（OpenAI 兼容）</span>
              <input
                type="text"
                value={aiBaseUrl}
                onChange={(e) => setAiBaseUrl(e.target.value)}
                placeholder="https://api.openai.com/v1"
                className="mt-1 w-full rounded-md border border-black/10 px-2 py-1.5 text-sm outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
              />
            </label>
            <label className="block">
              <span className="text-sm">API Key</span>
              <input
                type="password"
                value={aiApiKey}
                onChange={(e) => setAiApiKey(e.target.value)}
                placeholder={
                  aiConfig?.hasApiKey ? '已保存 · 输入可更换' : 'sk-…（只保存在本机）'
                }
                className="mt-1 w-full rounded-md border border-black/10 px-2 py-1.5 text-sm outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
              />
            </label>
            <label className="block">
              <span className="text-sm">模型名</span>
              <input
                type="text"
                value={aiModel}
                onChange={(e) => setAiModel(e.target.value)}
                placeholder="gpt-4o-mini / deepseek-chat / …"
                className="mt-1 w-full rounded-md border border-black/10 px-2 py-1.5 text-sm outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
              />
            </label>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => void saveAiConfig()}
                disabled={aiBusy}
                className="rounded-md bg-blue-500 px-3 py-1.5 text-sm text-white transition hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
              >
                保存
              </button>
              <button
                type="button"
                onClick={() => void testAiConfig()}
                disabled={aiBusy}
                className="rounded-md bg-zinc-100 px-3 py-1.5 text-sm text-zinc-700 transition hover:bg-zinc-200 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-zinc-700 dark:text-zinc-200 dark:hover:bg-zinc-600"
              >
                测试连接
              </button>
              <button
                type="button"
                onClick={() => void aiService.openChatWindow()}
                className="ml-auto rounded-md bg-zinc-100 px-3 py-1.5 text-sm text-zinc-700 transition hover:bg-zinc-200 dark:bg-zinc-700 dark:text-zinc-200 dark:hover:bg-zinc-600"
              >
                打开对话…
              </button>
            </div>
            {aiStatus && (
              <p className={`text-xs ${aiStatus.ok ? 'text-emerald-500' : 'text-red-500'}`}>
                {aiStatus.text}
              </p>
            )}
          </div>
          <p className="mt-2 text-xs leading-relaxed text-zinc-400">
            兼容 OpenAI / DeepSeek 等标准 /chat/completions 接口。桌宠聊天时会自动带上你的
            真实待办与专注数据，回答更贴合现状；配置只存本机 SQLite，不会上传。
          </p>
        </section>

          </div>
        </div>

        <p className="text-xs leading-relaxed text-zinc-400">
          数据保存在本机 SQLite 数据库（%APPDATA%/com.floatdo.app/floatdo.db），
          不会上传到任何服务器。桌宠的照片、风格和人格请到「桌宠中心」配置。
        </p>
      </div>
    </div>
  );
}
