import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { SettingsPage } from '../settings/SettingsPage';
import { StatsPage } from '../stats/StatsPage';
import { ChatPage } from '../chat/ChatPage';
import { PetCenterPage } from '../pet/PetCenterPage';
import { TaskCenterPage } from '../tasks/TaskCenterPage';
import { PANEL_OPENED_EVENT } from '../../services/settingsService';

type PanelTab = 'tasks' | 'settings' | 'stats' | 'chat' | 'pet';

/** 主导航：统计 / 任务 / AI 对话 / 桌宠中心；设置单独沉在侧边栏底部。 */
const NAV_ITEMS: { key: PanelTab; icon: string; label: string }[] = [
  { key: 'stats', icon: '📊', label: '统计' },
  { key: 'tasks', icon: '📋', label: '任务' },
  { key: 'chat', icon: '🐱', label: 'AI 对话' },
  { key: 'pet', icon: '🐾', label: '桌宠中心' },
];

const SETTINGS_ITEM: { key: PanelTab; icon: string; label: string } = {
  key: 'settings',
  icon: '⚙️',
  label: '设置',
};

/**
 * 主面板：左侧导航 + 右侧内容区，设置 / 统计共用一个窗口。
 * 托盘「设置」「统计」都打开这里并切到对应页签；以后新增区块
 * 只要在 NAV_ITEMS 加一项、App 里把内容组件挂进来即可。
 */
export function MainShell() {
  const [tab, setTab] = useState<PanelTab>('stats');
  // 每次被打开（panel://open）时 +1，作为内容区 key 强制重挂载，保证数据最新
  const [openSeq, setOpenSeq] = useState(0);

  useEffect(() => {
    const unlisten = listen<{ tab: string }>(PANEL_OPENED_EVENT, (e) => {
      if (NAV_ITEMS.some((item) => item.key === e.payload.tab)) {
        setTab(e.payload.tab as PanelTab);
      }
      setOpenSeq((n) => n + 1);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className="flex h-screen bg-zinc-100 text-zinc-800 dark:bg-zinc-900 dark:text-zinc-200">
      <nav className="flex w-36 shrink-0 flex-col gap-1 border-r border-black/10 bg-white p-3 dark:border-white/10 dark:bg-zinc-800">
        <div className="mb-3 px-2 text-base font-semibold">FloatDo</div>
        {NAV_ITEMS.map((item) => (
          <button
            key={item.key}
            onClick={() => setTab(item.key)}
            className={`flex items-center gap-2 rounded-lg px-3 py-2 text-sm transition ${
              tab === item.key
                ? 'bg-blue-500 text-white'
                : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-700'
            }`}
          >
            <span aria-hidden>{item.icon}</span>
            {item.label}
          </button>
        ))}
        {/* 设置与主导航分组，沉在侧边栏底部 */}
        <button
          onClick={() => setTab(SETTINGS_ITEM.key)}
          className={`mt-auto flex items-center gap-2 rounded-lg px-3 py-2 text-sm transition ${
            tab === SETTINGS_ITEM.key
              ? 'bg-blue-500 text-white'
              : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-700'
          }`}
        >
          <span aria-hidden>{SETTINGS_ITEM.icon}</span>
          {SETTINGS_ITEM.label}
        </button>
      </nav>
      <main className="min-w-0 flex-1 overflow-hidden">
        {tab === 'stats' ? (
          <div className="h-full overflow-y-auto">
            <StatsPage key={`stats-${openSeq}`} />
          </div>
        ) : tab === 'chat' ? (
          <ChatPage key={`chat-${openSeq}`} />
        ) : tab === 'pet' ? (
          <div className="h-full overflow-y-auto">
            <PetCenterPage key={`pet-${openSeq}`} />
          </div>
        ) : tab === 'tasks' ? (
          <div className="h-full overflow-y-auto">
            <TaskCenterPage key={`tasks-${openSeq}`} />
          </div>
        ) : (
          <div className="h-full overflow-y-auto">
            <SettingsPage key={`settings-${openSeq}`} />
          </div>
        )}
      </main>
    </div>
  );
}
