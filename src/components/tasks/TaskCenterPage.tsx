import { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTaskStore } from '../../stores/taskStore';
import { TaskItem } from '../task/TaskItem';
import { TaskForm } from '../task/TaskForm';
import { PET_TASKS_CHANGED_EVENT } from '../../services/petService';
import { compareByImportance, isPending } from '../../utils/priority';
import { isOverdue, isToday } from '../../utils/time';
import type { Task } from '../../types';

/**
 * 任务页（主面板）：按「逾期 → 今天（未完成 / 已完成）→ 未来（按天）→ 没有截止时间」
 * 分块展示全部待办视角的任务。历史完成记录去「统计」页查（有搜索和筛选）。
 */
export function TaskCenterPage() {
  const { tasks, load } = useTaskStore();
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    void load();
    // 悬浮窗里勾选/增删任务时实时同步（后端在任务变化时都会发这个事件）
    const unlisten = listen(PET_TASKS_CHANGED_EVENT, () => {
      void load();
    });
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const groups = useMemo(() => groupTasks(tasks), [tasks]);

  return (
    <div className="p-6">
      <div className="mx-auto max-w-4xl space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-lg font-semibold leading-tight">任务</h1>
            <p className="mt-0.5 text-xs text-zinc-400">
              未完成 {groups.pendingTotal} · 今天已完成 {groups.doneToday.length}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setAdding((v) => !v)}
            className={`rounded-lg px-3 py-1.5 text-sm transition ${
              adding
                ? 'bg-zinc-200 text-zinc-600 hover:bg-zinc-300 dark:bg-zinc-700 dark:text-zinc-300'
                : 'bg-blue-500 text-white hover:bg-blue-600'
            }`}
          >
            {adding ? '收起表单' : '+ 添加任务'}
          </button>
        </div>

        {adding && (
          <section className="rounded-xl bg-white p-2 shadow-sm dark:bg-zinc-800">
            <TaskForm onDone={() => setAdding(false)} />
          </section>
        )}

        {groups.totalCount === 0 && (
          <div className="rounded-xl bg-white p-8 text-center text-sm text-zinc-400 shadow-sm dark:bg-zinc-800">
            还没有任何任务，去悬浮窗点「+ 添加任务」创建一个吧
          </div>
        )}

        {/* 双栏：左边「当下要做的事」，右边「往后安排的事」 */}
        <div className="grid grid-cols-2 items-start gap-4">
          <div className="space-y-4">
            <TaskSection
              title="逾期"
              icon="🔥"
              accent="text-red-500"
              tasks={groups.overdue}
              emptyHint="没有逾期任务，保持住！"
            />
            <TaskSection
              title="今天 · 未完成"
              icon="📅"
              tasks={groups.todayPending}
              emptyHint="今天没有安排中的任务"
            />
            <TaskSection
              title="今天 · 已完成"
              icon="✅"
              accent="text-emerald-600 dark:text-emerald-400"
              tasks={groups.doneToday}
              emptyHint="今天还没有完成的任务"
            />
          </div>

          <div className="space-y-4">
            {groups.future.length > 0 ? (
              groups.future.map(([label, list]) => (
                <TaskSection key={label} title={label} icon="🔜" tasks={list} emptyHint="" />
              ))
            ) : (
              <TaskSection
                title="未来"
                icon="🔜"
                tasks={[]}
                emptyHint="没有安排在未来的任务，给明天加点计划吧"
              />
            )}
            <TaskSection
              title="没有截止时间"
              icon="🗂"
              tasks={groups.noDue}
              emptyHint="所有任务都安排上了时间"
            />
          </div>
        </div>

        <p className="text-xs leading-relaxed text-zinc-400">
          需要找历史完成记录？「统计」页右侧有全部任务记录，支持搜索、完成/逾期状态和日期范围筛选。
        </p>
      </div>
    </div>
  );
}

interface TaskGroups {
  overdue: Task[];
  todayPending: Task[];
  doneToday: Task[];
  future: [string, Task[]][];
  noDue: Task[];
  pendingTotal: number;
  totalCount: number;
}

/** 纯函数分组：逾期 / 今天未完成 / 今天已完成 / 未来按天 / 无截止时间。 */
function groupTasks(tasks: Task[]): TaskGroups {
  const pending = tasks.filter(isPending).sort(compareByImportance);
  const doneToday = tasks
    .filter((t) => t.status === 'COMPLETED' && isToday(t.completedAt))
    .sort((a, b) => (b.completedAt ?? '').localeCompare(a.completedAt ?? ''));

  const overdue = pending.filter((t) => isOverdue(t.dueAt));
  const todayPending = pending.filter(
    (t) => !isOverdue(t.dueAt) && isToday(t.dueAt),
  );
  const futurePending = pending.filter(
    (t) => t.dueAt !== null && !isOverdue(t.dueAt) && !isToday(t.dueAt),
  );
  const noDue = pending.filter((t) => t.dueAt === null);

  // 未来任务按本地日期分组，组内保持重要度排序
  const byDay = new Map<string, Task[]>();
  for (const t of futurePending) {
    const d = new Date(t.dueAt as string);
    const key = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    const list = byDay.get(key) ?? [];
    list.push(t);
    byDay.set(key, list);
  }
  const future: [string, Task[]][] = [...byDay.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, list]) => [dayLabel(key), list]);

  return {
    overdue,
    todayPending,
    doneToday,
    future,
    noDue,
    pendingTotal: pending.length,
    totalCount: tasks.length,
  };
}

/** 未来的某天 → 「明天」「后天」或「8月31日 · 周一」。 */
function dayLabel(key: string): string {
  const [y, m, d] = key.split('-').map(Number);
  const target = new Date(y, m - 1, d);
  const today = new Date();
  const dayMs = 24 * 60 * 60 * 1000;
  const diff = Math.round(
    (new Date(target).setHours(0, 0, 0, 0) - new Date(today).setHours(0, 0, 0, 0)) / dayMs,
  );
  if (diff === 1) return '明天';
  if (diff === 2) return '后天';
  const week = '日一二三四五六'[target.getDay()];
  return `${target.getMonth() + 1}月${target.getDate()}日 · 周${week}`;
}

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

function TaskSection({
  title,
  icon,
  tasks,
  accent,
  emptyHint,
}: {
  title: string;
  icon: string;
  tasks: Task[];
  accent?: string;
  emptyHint: string;
}) {
  if (tasks.length === 0 && !emptyHint) return null;
  return (
    <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
      <h2 className="mb-2 flex items-center gap-1.5 text-sm font-medium">
        <span aria-hidden>{icon}</span>
        <span className={accent}>{title}</span>
        {tasks.length > 0 && (
          <span className="text-xs font-normal text-zinc-400">{tasks.length}</span>
        )}
      </h2>
      {tasks.length > 0 ? (
        <div className="-mx-1">
          {tasks.map((t) => (
            <TaskItem key={t.id} task={t} />
          ))}
        </div>
      ) : (
        <p className="py-1 text-sm text-zinc-400 dark:text-zinc-500">{emptyHint}</p>
      )}
    </section>
  );
}
