import { useEffect, useState } from 'react';
import { useFocusStore, formatCountdown } from '../../stores/focusStore';
import { useTaskStore } from '../../stores/taskStore';
import { isPending, compareByImportance } from '../../utils/priority';

/** 每隔 interval 毫秒跳动一次的「当前时间」，用于倒计时展示。 */
export function useNow(intervalMs = 500): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(timer);
  }, [intervalMs]);
  return now;
}

/**
 * 展开态底部的专注面板：
 * - 空闲：选任务（可不绑）→ 开始专注；显示今日累计
 * - 专注：倒计时 + 进度条 + 停止
 * - 休息：倒计时 + 跳过休息
 * 计时在后端，这里只做展示；阶段切换由 focus://changed 事件驱动。
 */
export function FocusPanel() {
  const { phase, endsAt, session, workMinutes, todaySeconds, start, stop } =
    useFocusStore();
  const { tasks } = useTaskStore();
  const now = useNow(500);

  const pending = tasks.filter(isPending).sort(compareByImportance);
  const [taskId, setTaskId] = useState<string>('');

  // 剩余秒数按后端给的结束时间倒推，误差只有本机时钟偏差
  const remaining = endsAt
    ? Math.max(0, Math.round((new Date(endsAt).getTime() - now) / 1000))
    : 0;

  if (phase === 'FOCUS' && endsAt) {
    const boundTask = tasks.find((t) => t.id === session?.taskId);
    const progress =
      1 - remaining / Math.max(1, workMinutes * 60);
    return (
      <div className="px-2.5">
        <div className="flex items-center gap-2 py-1">
          <span className="shrink-0 rounded-md bg-blue-500/10 px-1.5 py-0.5 text-sm font-semibold tabular-nums text-blue-600 dark:text-blue-400">
            {formatCountdown(remaining)}
          </span>
          <span className="min-w-0 flex-1 truncate text-xs text-zinc-500 dark:text-zinc-400">
            专注中{boundTask ? ` · ${boundTask.title}` : ''}
          </span>
          <button
            onClick={() => void stop()}
            className="shrink-0 rounded-md px-2 py-0.5 text-xs text-zinc-500 transition hover:bg-black/5 dark:text-zinc-400 dark:hover:bg-white/10"
          >
            停止
          </button>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-black/5 dark:bg-white/10">
          <div
            className="h-full rounded-full bg-blue-500 transition-[width] duration-500"
            style={{ width: `${Math.min(100, progress * 100)}%` }}
          />
        </div>
      </div>
    );
  }

  if (phase === 'BREAK' && endsAt) {
    return (
      <div className="flex items-center gap-2 px-3 py-1.5">
        <span className="shrink-0 rounded-md bg-emerald-500/10 px-1.5 py-0.5 text-sm font-semibold tabular-nums text-emerald-600 dark:text-emerald-400">
          {formatCountdown(remaining)}
        </span>
        <span className="flex-1 text-xs text-zinc-500 dark:text-zinc-400">
          ☕ 休息一下，眼睛离开屏幕
        </span>
        <button
          onClick={() => void stop()}
          className="shrink-0 rounded-md px-2 py-0.5 text-xs text-zinc-500 transition hover:bg-black/5 dark:text-zinc-400 dark:hover:bg-white/10"
        >
          跳过
        </button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 px-3 py-1.5">
      <button
        onClick={() => void start(taskId === '' ? null : Number(taskId))}
        className="shrink-0 rounded-md bg-blue-500 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-blue-600"
      >
        🎯 开始专注
      </button>
      <select
        value={taskId}
        onChange={(e) => setTaskId(e.target.value)}
        className="min-w-0 flex-1 rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-xs text-zinc-600 outline-none dark:border-white/10 dark:text-zinc-300"
      >
        <option value="">不绑定任务</option>
        {pending.slice(0, 8).map((t) => (
          <option key={t.id} value={t.id}>
            {t.title}
          </option>
        ))}
      </select>
      <span className="shrink-0 text-xs tabular-nums text-zinc-400">
        今日 {Math.round(todaySeconds / 60)} 分钟
      </span>
    </div>
  );
}
