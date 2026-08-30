import type { Task } from '../../types';
import { useTaskStore } from '../../stores/taskStore';
import { useUiStore } from '../../stores/uiStore';
import { PRIORITY_META, isPending } from '../../utils/priority';
import { formatDue, isOverdue } from '../../utils/time';
import { TaskForm } from './TaskForm';

/** 展开态中的单行任务：勾选完成、编辑、推迟、删除。 */
export function TaskItem({ task }: { task: Task }) {
  const { toggleComplete, remove, snooze, setError } = useTaskStore();
  const editingId = useUiStore((s) => s.editingId);
  const setEditingId = useUiStore((s) => s.setEditingId);
  const editing = editingId === task.id;

  if (editing) {
    return <TaskForm task={task} onDone={() => setEditingId(null)} />;
  }

  const done = task.status === 'COMPLETED';
  const overdue = isPending(task) && isOverdue(task.dueAt);
  const dot = PRIORITY_META[task.priority].color;

  return (
    <div className="group flex items-center gap-2 rounded-lg px-2 py-1.5 transition hover:bg-black/5 dark:hover:bg-white/5">
      <button
        onClick={() => void toggleComplete(task).catch((e) => setError(String(e)))}
        aria-label={done ? '标记为未完成' : '完成任务'}
        className={`flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded-full border text-[10px] transition ${
          done
            ? 'border-emerald-500 bg-emerald-500 text-white'
            : 'border-zinc-400 hover:border-emerald-500 dark:border-zinc-500'
        }`}
      >
        {done ? '✓' : ''}
      </button>

      <span
        className="h-2.5 w-2.5 shrink-0 rounded-full"
        style={{ backgroundColor: dot }}
        title={`优先级：${PRIORITY_META[task.priority].label}`}
      />

      <button
        onClick={() => setEditingId(task.id)}
        className={`min-w-0 flex-1 truncate text-left text-sm transition ${
          done
            ? 'text-zinc-400 line-through dark:text-zinc-600'
            : 'text-zinc-800 dark:text-zinc-200'
        }`}
        title={task.title}
      >
        {task.title}
      </button>

      {task.dueAt && (
        <span
          className={`shrink-0 text-xs tabular-nums ${
            overdue ? 'font-medium text-red-500' : 'text-zinc-500 dark:text-zinc-400'
          }`}
        >
          {formatDue(task.dueAt)}
        </span>
      )}

      {/* 已完成的任务没有「推迟」的意义，也不提供删除（先取消完成再删，防误触） */}
      {!done && (
        <div className="hidden shrink-0 items-center gap-0.5 group-hover:flex">
          <button
            onClick={() => void snooze(task).catch((e) => setError(String(e)))}
            title="推迟到明天"
            className="rounded px-1 text-xs text-zinc-500 hover:bg-black/10 dark:hover:bg-white/10"
          >
            ⏭
          </button>
          <button
            onClick={() => void remove(task.id).catch((e) => setError(String(e)))}
            title="删除"
            className="rounded px-1 text-xs text-zinc-500 hover:bg-red-100 hover:text-red-500 dark:hover:bg-red-500/20"
          >
            🗑
          </button>
        </div>
      )}
    </div>
  );
}
