import { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Task } from '../../types';
import {
  WIDGET_COLLAPSED_HEIGHT,
  resizeWidget,
} from '../../utils/widgetWindow';
import { useTaskStore } from '../../stores/taskStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { useUiStore } from '../../stores/uiStore';
import {
  isPending,
  pickNextTask,
  pickUrgentTasks,
  todayStats,
  PRIORITY_META,
  compareByPriorityThenOverdue,
} from '../../utils/priority';
import { formatDue, isOverdue } from '../../utils/time';
import { SETTINGS_CHANGED_EVENT } from '../../services/settingsService';
import { statsService } from '../../services/statsService';
import { PET_TASKS_CHANGED_EVENT } from '../../services/petService';
import {
  FOCUS_CHANGED_EVENT,
  type FocusPhase,
  type FocusState,
} from '../../services/focusService';
import { useFocusStore, formatCountdown } from '../../stores/focusStore';
import { taskService } from '../../services/taskService';
import { TaskItem } from '../task/TaskItem';
import { TaskForm } from '../task/TaskForm';
import { FocusPanel, useNow } from './FocusPanel';
import { useWidgetDrag } from './useWidgetDrag';

const ROW_HEIGHT = 34;
const LIST_MAX_HEIGHT = 240;
/** 折叠态：表头高度 + 每行任务高度（用于随行数自适应收缩窗口高度）。 */
const COLLAPSED_HEADER_H = 36;
const COLLAPSED_ROW_H = 27;

/** 悬浮窗主页面：折叠 ⇄ 展开全部状态驱动，窗口尺寸随内容同步调整。 */
export function WidgetPage() {
  const { tasks, error, load, reorder } = useTaskStore();
  const { autoExpand, opacity, load: loadSettings } = useSettingsStore();
  const { phase: focusPhase, endsAt: focusEndsAt, refresh: refreshFocus, apply: applyFocus } = useFocusStore();
  const { expanded, setExpanded, setEditingId, editingId, adding, setAdding } = useUiStore();

  useEffect(() => {
    void load();
    void loadSettings();
    void refreshFocus();
    // 托盘「添加任务」：显示并展开 + 打开添加表单
    const unexpand = listen('widget://expand-add', () => {
      setAdding(true);
      setExpanded(true);
    });
    // 设置窗口修改后同步
    const unsettings = listen(SETTINGS_CHANGED_EVENT, () => {
      void loadSettings();
    });
    // 主面板/桌宠侧增删改任务后同步（悬浮窗是独立 store，必须重拉）
    const untasks = listen(PET_TASKS_CHANGED_EVENT, () => {
      void load();
    });
    // 专注阶段切换（后端推送，含启动恢复）
    const unfocus = listen<FocusState>(FOCUS_CHANGED_EVENT, (e) => {
      applyFocus(e.payload);
    });
    return () => {
      unexpand.then((f) => f());
      unsettings.then((f) => f());
      untasks.then((f) => f());
      unfocus.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pending = useMemo(() => tasks.filter(isPending), [tasks]);
  const stats = useMemo(() => todayStats(tasks), [tasks]);
  const overdueCount = useMemo(
    () => pending.filter((t) => isOverdue(t.dueAt)).length,
    [pending],
  );
  // 展开列表按优先级从高到低；同优先级内逾期置顶，其余保持手动拖拽的顺序
  const displayTasks = useMemo(
    () =>
      tasks
        .filter(isPending)
        .slice()
        .sort(compareByPriorityThenOverdue)
        .slice(0, 8),
    [tasks],
  );

  // 折叠态常驻内容：紧急任务全部置顶 + 紧急之外「接下来做」的一条
  // （有截止时间的取最近；都没截止时间取最先创建的待办）
  const collapsed = useMemo(() => {
    const urgentAll = pickUrgentTasks(tasks);
    const urgent = urgentAll.slice(0, 3);
    const next = pickNextTask(tasks, urgent);
    return {
      rows: [...urgent, ...(next ? [next] : [])],
      overflow: urgentAll.length - urgent.length,
    };
  }, [tasks]);
  const collapsedHeight = useMemo(
    () =>
      Math.max(
        WIDGET_COLLAPSED_HEIGHT,
        COLLAPSED_HEADER_H +
          (collapsed.rows.length + (collapsed.overflow > 0 ? 1 : 0)) * COLLAPSED_ROW_H +
          6,
      ),
    [collapsed],
  );

  // 展开态里专注面板占的高度：专注中有进度条略高，其余一行
  const focusPanelH = focusPhase === 'FOCUS' ? 56 : 40;
  // 列表被截断（超过 8 条）时底部多一行「显示全部」
  const truncated = pending.length > 8;

  const expandedHeight = useMemo(() => {
    const listH = Math.max(Math.min(displayTasks.length * ROW_HEIGHT, LIST_MAX_HEIGHT), 56);
    // 表单含快捷日期按钮行，比普通底栏高
    const footerH = adding ? 132 : 44;
    // 时间选择面板是紧凑尺寸（约 210px），加表单后窗口装得下，无需加高窗口——
    // 一旦加高，整个列表会被顶上去跳动
    return (
      40 + listH + footerH + focusPanelH + (error ? 20 : 0) + (truncated ? 26 : 0) + 12
    );
  }, [displayTasks.length, adding, error, focusPanelH, truncated]);

  // 展开时立刻扩大窗口（向上生长）；收起时等 CSS 动画播完再缩小，避免内容被裁切
  useEffect(() => {
    if (expanded) {
      void resizeWidget(expandedHeight);
      return;
    }
    const timer = setTimeout(() => void resizeWidget(collapsedHeight), 230);
    return () => clearTimeout(timer);
  }, [expanded, expandedHeight, collapsedHeight]);

  const collapse = () => {
    setExpanded(false);
    setAdding(false);
    setEditingId(null);
  };

  const handleMouseEnter = () => {
    if (autoExpand && !expanded) setExpanded(true);
  };

  const handleMouseLeave = () => {
    // 正在输入时不自动收起
    if (adding || editingId !== null) return;
    if (expanded) collapse();
  };

  // 键盘无障碍：窗口聚焦时 Enter/空格 展开，Escape 收起
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && expanded) {
        collapse();
      } else if ((e.key === 'Enter' || e.key === ' ') && !expanded) {
        setExpanded(true);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [expanded]);

  // 折叠条上专注倒计时（每 0.5s 跳一次；专注与休息都显示）
  const now = useNow(500);
  const focusRemaining =
    focusEndsAt && focusPhase !== 'IDLE'
      ? Math.max(0, Math.round((new Date(focusEndsAt).getTime() - now) / 1000))
      : null;

  const onDragMouseDown = useWidgetDrag();

  return (
    <div className="fixed inset-0 overflow-hidden">
      <div
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        onMouseDown={onDragMouseDown}
        className="absolute inset-x-0 bottom-0 rounded-xl bg-white/95 shadow-lg ring-1 ring-black/10 backdrop-blur-md dark:bg-zinc-900/95 dark:ring-white/10"
        style={{
          height: expanded ? expandedHeight : collapsedHeight,
          opacity,
          transition: 'height 200ms ease-out',
        }}
      >
        {expanded ? (
          <Expanded
            stats={stats}
            displayTasks={displayTasks}
            pendingCount={pending.length}
            truncated={truncated}
            error={error}
            onCollapse={collapse}
            onReorder={reorder}
          />
        ) : (
          <Collapsed
            stats={stats}
            overdueCount={overdueCount}
            rows={collapsed.rows}
            overflow={collapsed.overflow}
            focusRemaining={focusRemaining}
            focusPhase={focusPhase}
          />
        )}
      </div>
    </div>
  );
}

type Stats = { done: number; total: number };

/** 截止时间标签：逾期红色加粗，其余弱化灰。 */
function DueTag({ task }: { task: Task }) {
  if (!task.dueAt) return null;
  const overdue = task.status !== 'COMPLETED' && new Date(task.dueAt).getTime() < Date.now();
  return (
    <span
      className={`shrink-0 text-xs tabular-nums ${
        overdue
          ? 'font-medium text-red-500'
          : 'text-zinc-500 dark:text-zinc-400'
      }`}
    >
      {formatDue(task.dueAt)}
    </span>
  );
}

function Collapsed({
  stats,
  overdueCount,
  rows,
  overflow,
  focusRemaining,
  focusPhase,
}: {
  stats: Stats;
  overdueCount: number;
  rows: Task[];
  overflow: number;
  focusRemaining: number | null;
  focusPhase: FocusPhase;
}) {
  return (
    <div className="flex h-full flex-col px-3">
      <div className="flex items-center justify-between pt-2">
        <span className="text-xs font-medium text-zinc-500 dark:text-zinc-400">今日任务</span>
        <div className="flex items-center gap-1.5">
          {overdueCount > 0 && (
            <span
              title={`${overdueCount} 条任务已逾期`}
              className="rounded-full bg-red-500/10 px-1.5 py-0.5 text-xs font-semibold tabular-nums text-red-500"
            >
              逾期 {overdueCount}
            </span>
          )}
          <span className="text-sm font-semibold tabular-nums text-zinc-700 dark:text-zinc-200">
            {stats.done}/{stats.total}
          </span>
          <span className="text-[10px] text-zinc-400">⌃</span>
        </div>
      </div>
      <div className="mt-1.5 flex min-w-0 flex-col pb-1">
        {focusRemaining !== null ? (
          <div className="flex min-w-0 items-center gap-1.5 py-0.5">
            <span
              className={`shrink-0 rounded px-1 py-0.5 text-xs font-semibold tabular-nums ${
                focusPhase === 'FOCUS'
                  ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400'
                  : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
              }`}
            >
              {formatCountdown(focusRemaining)}
            </span>
            <span className="min-w-0 flex-1 truncate text-sm text-zinc-500 dark:text-zinc-400">
              {focusPhase === 'FOCUS' ? '专注中' : '休息中'}
            </span>
          </div>
        ) : rows.length === 0 ? (
          <span className="py-0.5 text-sm text-zinc-400 dark:text-zinc-500">
            暂无任务，享受当下 ☕
          </span>
        ) : (
          <>
            {rows.map((t) => (
              <div key={t.id} className="flex min-w-0 items-center gap-1.5 py-0.5">
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-full"
                  style={{ backgroundColor: PRIORITY_META[t.priority].color }}
                />
                <span className="min-w-0 flex-1 truncate text-sm text-zinc-800 dark:text-zinc-200">
                  {t.title}
                </span>
                <DueTag task={t} />
              </div>
            ))}
            {overflow > 0 && (
              <div className="py-0.5 text-xs text-zinc-400">还有 {overflow} 个紧急任务…</div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function Expanded({
  stats,
  displayTasks,
  pendingCount,
  truncated,
  error,
  onCollapse,
  onReorder,
}: {
  stats: Stats;
  displayTasks: ReturnType<typeof useTaskStore.getState>['tasks'];
  pendingCount: number;
  truncated: boolean;
  error: string | null;
  onCollapse: () => void;
  onReorder: (orderedIds: number[]) => Promise<void>;
}) {
  const adding = useUiStore((s) => s.adding);
  const setAdding = useUiStore((s) => s.setAdding);
  // 拖拽排序状态必须放在列表层级：放到行组件里的话，
  // 目标行的 onDrop 读不到「源行是谁」，永远无法换位
  const [dragId, setDragId] = useState<number | null>(null);
  const [overId, setOverId] = useState<number | null>(null);

  const dropOn = (targetId: number) => {
    if (dragId !== null && dragId !== targetId) {
      // 取待办全量顺序，把拖动项移动到目标位置，交给 store 持久化
      const ids = useTaskStore
        .getState()
        .tasks.filter((t) => t.status === 'TODO' || t.status === 'IN_PROGRESS')
        .map((t) => t.id);
      const from = ids.indexOf(dragId);
      const to = ids.indexOf(targetId);
      if (from >= 0 && to >= 0) {
        ids.splice(from, 1);
        ids.splice(to, 0, dragId);
        void onReorder(ids);
      }
    }
    setDragId(null);
    setOverId(null);
  };

  return (
    <div className="flex h-full flex-col">
      <header
        className="flex items-center justify-between px-3 pb-1 pt-2.5"
      >
        <span className="text-xs font-medium text-zinc-500 dark:text-zinc-400">
          今日任务{pendingCount > stats.total ? ` · 共 ${pendingCount} 待办` : ''}
        </span>
        <div className="flex items-center gap-1.5">
          <span className="text-sm font-semibold tabular-nums text-zinc-700 dark:text-zinc-200">
            {stats.done}/{stats.total}
          </span>
          <button
            onClick={() => void statsService.openWindow()}
            title="打开主面板（统计）"
            className="rounded px-1 text-xs text-zinc-400 transition hover:bg-black/5 hover:text-zinc-600 dark:hover:bg-white/10"
          >
            📊
          </button>
          <button
            onClick={onCollapse}
            title="收起"
            className="rounded px-1 text-xs text-zinc-400 transition hover:bg-black/5 hover:text-zinc-600 dark:hover:bg-white/10"
          >
            ⌄
          </button>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-1.5">
        {displayTasks.map((t) => (
          <div
            key={t.id}
            data-task-drag
            draggable
            onDragStart={(e) => {
              setDragId(t.id);
              e.dataTransfer.effectAllowed = 'move';
              // Chromium 要求拖拽携带数据，否则部分场景 drop 不触发
              e.dataTransfer.setData('text/plain', String(t.id));
            }}
            onDragOver={(e) => {
              e.preventDefault();
              if (dragId !== null && dragId !== t.id) setOverId(t.id);
            }}
            onDragLeave={() => setOverId((v) => (v === t.id ? null : v))}
            onDrop={(e) => {
              e.preventDefault();
              dropOn(t.id);
            }}
            onDragEnd={() => {
              setDragId(null);
              setOverId(null);
            }}
            className={`cursor-grab rounded-lg transition active:cursor-grabbing ${
              dragId === t.id
                ? 'opacity-40'
                : overId === t.id
                  ? 'ring-1 ring-blue-400'
                  : ''
            }`}
            title="拖动调整顺序"
          >
            <TaskItem task={t} />
          </div>
        ))}
        {displayTasks.length === 0 && (
          <div className="px-3 py-5 text-center text-sm text-zinc-400 dark:text-zinc-500">
            暂无待办，享受当下 ☕
          </div>
        )}
      </div>

      {truncated && (
        <button
          onClick={() => void taskService.openCenter()}
          title="打开主面板的任务页"
          className="mx-2 rounded-lg bg-black/5 py-1 text-center text-xs text-zinc-500 transition hover:bg-black/10 dark:bg-white/10 dark:text-zinc-300 dark:hover:bg-white/15"
        >
          还有 {pendingCount - displayTasks.length} 条 · 显示全部
        </button>
      )}

      {error && (
        <div className="px-3 pb-1 text-xs text-red-500" role="alert">
          {error}
        </div>
      )}

      {/* 专注模式：空闲/专注/休息三态面板 */}
      <FocusPanel />

      <footer className="px-2 pb-2">
        {adding ? (
          <TaskForm onDone={() => setAdding(false)} />
        ) : (
          <button
            onClick={() => setAdding(true)}
            className="w-full rounded-lg px-2 py-1.5 text-left text-sm text-zinc-500 transition hover:bg-black/5 dark:text-zinc-400 dark:hover:bg-white/5"
          >
            + 添加任务
          </button>
        )}
      </footer>
    </div>
  );
}
