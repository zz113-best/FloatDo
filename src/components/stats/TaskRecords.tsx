import { useEffect, useState } from 'react';
import { taskService } from '../../services/taskService';
import { statsService } from '../../services/statsService';
import type { Task, TaskPage } from '../../types';
import { PRIORITY_META } from '../../utils/priority';

const PAGE_SIZE = 10;

type TriState = 'all' | 'yes' | 'no';

const PRIORITY_OPTIONS = [
  { value: '', label: '优先级：全部' },
  { value: 'URGENT', label: '🔴 紧急' },
  { value: 'HIGH', label: '🟣 高' },
  { value: 'MEDIUM', label: '🔵 中' },
  { value: 'LOW', label: '🟢 低' },
];

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

/** RFC3339 → 本地 M/D HH:mm；空值显示 —。 */
function formatStamp(iso: string | null): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '—';
  return `${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * 全部任务记录：分页表格（每页 10 条，按创建时间新的在前）。
 * 筛选：关键词 + 完成/未完成 + 逾期/未逾期 + 截止日期范围 + 完成日期范围。
 */
export function TaskRecords() {
  const [keyword, setKeyword] = useState('');
  const [completed, setCompleted] = useState<TriState>('all');
  const [overdue, setOverdue] = useState<TriState>('all');
  const [priority, setPriority] = useState('');
  const [dueFrom, setDueFrom] = useState('');
  const [dueTo, setDueTo] = useState('');
  const [doneFrom, setDoneFrom] = useState('');
  const [doneTo, setDoneTo] = useState('');
  const [page, setPage] = useState(1);
  const [data, setData] = useState<TaskPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exportNote, setExportNote] = useState<string | null>(null);

  const exportCsv = async () => {
    setExportNote(null);
    try {
      const path = await statsService.exportTasksCsv({
        keyword,
        completed: completed === 'all' ? null : completed === 'yes',
        overdue: overdue === 'all' ? null : overdue === 'yes',
        priority: priority || null,
        dueFrom: dueFrom || null,
        dueTo: dueTo || null,
        completedFrom: doneFrom || null,
        completedTo: doneTo || null,
      });
      setExportNote(path ? `已导出 ${path}` : '');
    } catch (e) {
      setExportNote(null);
      setError(`导出失败: ${String(e)}`);
    }
  };

  useEffect(() => {
    // 关键词输入防抖；筛选条件变化后回到第 1 页（在 setFilter 里处理）
    const timer = setTimeout(() => {
      taskService
        .search({
          keyword,
          completed: completed === 'all' ? null : completed === 'yes',
          overdue: overdue === 'all' ? null : overdue === 'yes',
          priority: priority || null,
          dueFrom: dueFrom || null,
          dueTo: dueTo || null,
          completedFrom: doneFrom || null,
          completedTo: doneTo || null,
          page,
          pageSize: PAGE_SIZE,
        })
        .then((r) => {
          setData(r);
          setError(null);
        })
        .catch((e) => setError(String(e)));
    }, 250);
    return () => clearTimeout(timer);
  }, [keyword, completed, overdue, priority, dueFrom, dueTo, doneFrom, doneTo, page]);

  const setFilter = (apply: () => void) => {
    apply();
    setPage(1);
  };

  const hasFilter =
    keyword ||
    completed !== 'all' ||
    overdue !== 'all' ||
    priority ||
    dueFrom ||
    dueTo ||
    doneFrom ||
    doneTo;

  const totalPages = data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : 1;
  const items = data?.items ?? [];

  return (
    <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
      <div className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-medium">全部任务记录</h2>
        <div className="flex items-center gap-2">
          {exportNote && <span className="text-xs text-emerald-500">{exportNote}</span>}
          <button
            type="button"
            onClick={() => void exportCsv()}
            title="导出全部任务记录为 CSV 文件"
            className="rounded-md bg-zinc-100 px-2 py-1 text-xs text-zinc-600 transition hover:bg-zinc-200 dark:bg-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-600"
          >
            ⬇ 导出 CSV
          </button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <input
            type="text"
            value={keyword}
            onChange={(e) => setFilter(() => setKeyword(e.target.value))}
            placeholder="搜索任务…"
            className="min-w-36 flex-1 rounded-md border border-black/10 px-2 py-1 text-sm outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
          />
          <select
            value={completed}
            onChange={(e) => setFilter(() => setCompleted(e.target.value as TriState))}
            title="按完成状态筛选"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none dark:border-white/10"
          >
            <option value="all">完成状态：全部</option>
            <option value="yes">已完成</option>
            <option value="no">未完成</option>
          </select>
          <select
            value={overdue}
            onChange={(e) => setFilter(() => setOverdue(e.target.value as TriState))}
            title="按逾期状态筛选"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none dark:border-white/10"
          >
            <option value="all">逾期状态：全部</option>
            <option value="yes">已逾期</option>
            <option value="no">未逾期</option>
          </select>
          <select
            value={priority}
            onChange={(e) => setFilter(() => setPriority(e.target.value))}
            title="按优先级筛选"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none dark:border-white/10"
          >
            {PRIORITY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span className="text-xs text-zinc-400">截止</span>
          <input
            type="date"
            value={dueFrom}
            onChange={(e) => setFilter(() => setDueFrom(e.target.value))}
            title="最早截止日期"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none [color-scheme:light] dark:border-white/10 dark:[color-scheme:dark]"
          />
          <span className="text-xs text-zinc-400">至</span>
          <input
            type="date"
            value={dueTo}
            onChange={(e) => setFilter(() => setDueTo(e.target.value))}
            title="最晚截止日期"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none [color-scheme:light] dark:border-white/10 dark:[color-scheme:dark]"
          />
          <span className="ml-3 text-xs text-zinc-400">完成</span>
          <input
            type="date"
            value={doneFrom}
            onChange={(e) => setFilter(() => setDoneFrom(e.target.value))}
            title="最早完成日期"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none [color-scheme:light] dark:border-white/10 dark:[color-scheme:dark]"
          />
          <span className="text-xs text-zinc-400">至</span>
          <input
            type="date"
            value={doneTo}
            onChange={(e) => setFilter(() => setDoneTo(e.target.value))}
            title="最晚完成日期"
            className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none [color-scheme:light] dark:border-white/10 dark:[color-scheme:dark]"
          />
          {hasFilter && (
            <button
              type="button"
              onClick={() => setFilter(() => {
                setKeyword('');
                setCompleted('all');
                setOverdue('all');
                setPriority('');
                setDueFrom('');
                setDueTo('');
                setDoneFrom('');
                setDoneTo('');
              })}
              className="ml-auto rounded-md px-2 py-1 text-xs text-zinc-500 transition hover:bg-black/5 dark:text-zinc-400 dark:hover:bg-white/10"
            >
              重置
            </button>
          )}
        </div>
      </div>

      {error && <p className="mt-2 text-xs text-red-500">{error}</p>}

      <div className="mt-3 overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-black/10 text-xs text-zinc-400 dark:border-white/10">
              <th className="py-1.5 pr-2 font-medium">任务</th>
              <th className="py-1.5 pr-2 font-medium">截止时间</th>
              <th className="py-1.5 pr-2 font-medium">完成时间</th>
              <th className="py-1.5 font-medium">状态</th>
            </tr>
          </thead>
          <tbody>
            {items.map((t) => (
              <RecordRow key={t.id} task={t} />
            ))}
            {items.length === 0 && (
              <tr>
                <td colSpan={4} className="py-5 text-center text-sm text-zinc-400">
                  {data ? '没有符合条件的任务记录' : '加载中…'}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-3 flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
        <span>
          共 {data?.total ?? 0} 条 · 第 {data?.page ?? page}/{totalPages} 页
        </span>
        <div className="flex gap-1.5">
          <button
            type="button"
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page <= 1}
            className="rounded-md bg-zinc-100 px-2.5 py-1 transition hover:bg-zinc-200 disabled:cursor-not-allowed disabled:opacity-40 dark:bg-zinc-700 dark:hover:bg-zinc-600"
          >
            上一页
          </button>
          <button
            type="button"
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page >= totalPages}
            className="rounded-md bg-zinc-100 px-2.5 py-1 transition hover:bg-zinc-200 disabled:cursor-not-allowed disabled:opacity-40 dark:bg-zinc-700 dark:hover:bg-zinc-600"
          >
            下一页
          </button>
        </div>
      </div>
      <p className="mt-2 text-xs leading-relaxed text-zinc-400">
        记录按创建时间排列（新的在前）。「已逾期」含未完成且过期、以及逾期后才完成的任务；截止/完成日期范围分别只对对应时间生效。
      </p>
    </section>
  );
}

function RecordRow({ task }: { task: Task }) {
  const done = task.status === 'COMPLETED';
  const duePassed = task.dueAt !== null && new Date(task.dueAt).getTime() < Date.now();
  const pendingOverdue = !done && duePassed;
  // 逾期完成：完成时间晚于截止时间（与后端筛选同口径）
  const lateDone =
    done &&
    task.dueAt !== null &&
    task.completedAt !== null &&
    new Date(task.completedAt).getTime() > new Date(task.dueAt).getTime();
  return (
    <tr className="border-b border-black/5 last:border-0 dark:border-white/5">
      <td className="max-w-56 py-1.5 pr-2">
        <div className="flex items-center gap-1.5">
          <span
            className="h-2.5 w-2.5 shrink-0 rounded-full"
            style={{ backgroundColor: PRIORITY_META[task.priority].color }}
            title={`优先级：${PRIORITY_META[task.priority].label}`}
          />
          <span className={`truncate ${done ? 'text-zinc-400 line-through dark:text-zinc-500' : ''}`}>
            {task.title}
          </span>
        </div>
      </td>
      <td className="py-1.5 pr-2 tabular-nums text-zinc-500 dark:text-zinc-400">
        {formatStamp(task.dueAt)}
      </td>
      <td className="py-1.5 pr-2 tabular-nums text-zinc-500 dark:text-zinc-400">
        {formatStamp(task.completedAt)}
      </td>
      <td className="py-1.5">
        {done ? (
          lateDone ? (
            <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-xs text-amber-600 dark:text-amber-400">
              逾期完成
            </span>
          ) : (
            <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-600 dark:text-emerald-400">
              已完成
            </span>
          )
        ) : pendingOverdue ? (
          <span className="rounded-full bg-red-500/10 px-2 py-0.5 text-xs text-red-500">逾期</span>
        ) : (
          <span className="rounded-full bg-zinc-500/10 px-2 py-0.5 text-xs text-zinc-500 dark:text-zinc-400">
            待完成
          </span>
        )}
      </td>
    </tr>
  );
}
