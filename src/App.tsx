import { getCurrentWindow } from '@tauri-apps/api/window';
import { WidgetPage } from './components/todo-widget/WidgetPage';
import { MainShell } from './components/shell/MainShell';
import { PetPage } from './components/pet/PetPage';

/**
 * 单一前端同时服务多个窗口，按 Tauri 窗口 label 区分：
 * - widget：桌面悬浮 Todo 窗口
 * - settings：主面板窗口（设置 / 统计 / AI 对话，左侧导航切换）
 * - pet：桌宠窗口
 * 不依赖 URL hash（窗口创建时 hash 可能丢失）。
 */
export default function App() {
  const label = getCurrentWindow().label;
  if (label === 'settings') return <MainShell />;
  if (label === 'pet') return <PetPage />;
  return <WidgetPage />;
}
