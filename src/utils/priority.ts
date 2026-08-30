import type { Priority, Task } from '../types';
import { isOverdue, isToday } from './time';

/** 颜色按「色相拉开」选择，保证收缩条小圆点下一眼能区分：红 / 紫 / 蓝 / 绿。 */
export const PRIORITY_META: Record<Priority, { label: string; color: string; rank: number }> = {
  URGENT: { label: '紧急', color: '#ef4444', rank: 3 },
  HIGH: { label: '高', color: '#a855f7', rank: 2 },
  MEDIUM: { label: '中', color: '#3b82f6', rank: 1 },
  LOW: { label: '低', color: '#22c55e', rank: 0 },
};

/** 任务是否仍然待办（OVERDUE 状态在第一版由前端根据 dueAt 计算）。 */
export function isPending(t: Task): boolean {
  return t.status === 'TODO' || t.status === 'IN_PROGRESS';
}

/** 智能排序：逾期 > 截止时间近 > 优先级高 > 手动顺序。 */
export function compareByImportance(a: Task, b: Task): number {
  const aOver = isPending(a) && isOverdue(a.dueAt) ? 1 : 0;
  const bOver = isPending(b) && isOverdue(b.dueAt) ? 1 : 0;
  if (aOver !== bOver) return bOver - aOver;

  const aTime = a.dueAt ? new Date(a.dueAt).getTime() : Number.MAX_SAFE_INTEGER;
  const bTime = b.dueAt ? new Date(b.dueAt).getTime() : Number.MAX_SAFE_INTEGER;
  if (aTime !== bTime) return aTime - bTime;

  const pr = PRIORITY_META[b.priority].rank - PRIORITY_META[a.priority].rank;
  if (pr !== 0) return pr;

  return a.sortOrder - b.sortOrder;
}

/** 悬浮窗展开列表排序：优先级高在前；同优先级内逾期在前，其余保持手动拖拽顺序（stable sort）。 */
export function compareByPriorityThenOverdue(a: Task, b: Task): number {
  const pr = PRIORITY_META[b.priority].rank - PRIORITY_META[a.priority].rank;
  if (pr !== 0) return pr;
  const aOver = isOverdue(a.dueAt) ? 1 : 0;
  const bOver = isOverdue(b.dueAt) ? 1 : 0;
  if (aOver !== bOver) return bOver - aOver;
  return 0;
}

/** 悬浮窗折叠态展示的“当前最重要任务”。 */
export function pickTopTask(tasks: Task[]): Task | null {
  const pending = tasks.filter(isPending);
  if (pending.length === 0) return null;
  const todayOrOverdue = pending.filter(
    (t) => isOverdue(t.dueAt) || isToday(t.dueAt),
  );
  const pool = todayOrOverdue.length > 0 ? todayOrOverdue : pending;
  return [...pool].sort(compareByImportance)[0];
}

/** 折叠态常驻显示的紧急任务；逾期的不在这里显示（折叠条上只计入「逾期 N」徽标）。 */
export function pickUrgentTasks(tasks: Task[]): Task[] {
  return tasks
    .filter((t) => isPending(t) && t.priority === 'URGENT' && !isOverdue(t.dueAt))
    .sort(compareByImportance);
}

/**
 * 折叠态里紧急任务之外的「接下来做」：未逾期待办中截止时间最近的一条；
 * 都没有截止时间就取最先创建的待办。逾期任务不算。
 */
export function pickNextTask(tasks: Task[], exclude: Task[]): Task | null {
  const excluded = new Set(exclude.map((t) => t.id));
  const pool = tasks.filter(
    (t) => isPending(t) && !excluded.has(t.id) && !isOverdue(t.dueAt),
  );
  if (pool.length === 0) return null;
  const withDue = pool.filter((t) => t.dueAt);
  if (withDue.length > 0) {
    return [...withDue].sort(compareByImportance)[0];
  }
  return [...pool].sort(
    (a, b) => a.createdAt.localeCompare(b.createdAt) || a.sortOrder - b.sortOrder,
  )[0];
}

/**
 * 悬浮窗计数：今日已完成 / 全部待办（不止今天到期的——用户关心的是手里还剩多少活）。
 */
export function todayStats(tasks: Task[]): { done: number; total: number } {
  const todayDone = tasks.filter(
    (t) => t.status === 'COMPLETED' && isToday(t.completedAt),
  ).length;
  const pendingCount = tasks.filter(isPending).length;
  return { done: todayDone, total: todayDone + pendingCount };
}
