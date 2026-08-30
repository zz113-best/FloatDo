import { useEffect, useRef, useState } from 'react';
import type { Priority, Task } from '../../types';
import { useTaskStore } from '../../stores/taskStore';
import { PRIORITY_META } from '../../utils/priority';
import { isoToLocalInput, localInputToIso } from '../../utils/time';
import { useUiStore } from '../../stores/uiStore';

interface Props {
  task?: Task;
  onDone: () => void;
}

/** 快捷截止时间选项。 */
const QUICK_DUE: { label: string; days: number; hour: number; minute: number }[] = [
  { label: '今天 18:00', days: 0, hour: 18, minute: 0 },
  { label: '明天 9:00', days: 1, hour: 9, minute: 0 },
  { label: '明天 18:00', days: 1, hour: 18, minute: 0 },
  { label: '后天 9:00', days: 2, hour: 9, minute: 0 },
];

const WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六'];

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

interface DateTimeParts {
  y: number;
  /** 0-11 */
  m: number;
  d: number;
  h: number;
  min: number;
}

function partsOf(date: Date): DateTimeParts {
  return { y: date.getFullYear(), m: date.getMonth(), d: date.getDate(), h: date.getHours(), min: date.getMinutes() };
}

/** 新建 / 编辑任务的表单。task 为空表示新建。 */
export function TaskForm({ task, onDone }: Props) {
  const { add, update } = useTaskStore();
  const setAdding = useUiStore((s) => s.setAdding);
  const [title, setTitle] = useState(task?.title ?? '');
  const [priority, setPriority] = useState<Priority>(task?.priority ?? 'LOW');
  const [dueLocal, setDueLocal] = useState(isoToLocalInput(task?.dueAt ?? null));
  // 自绘日期时间选择器：原生日历下拉会弹到屏幕外（悬浮窗贴屏幕底边），必须自己画
  const [pickerOpen, setPickerOpen] = useState(false);
  const [sel, setSel] = useState<DateTimeParts>(() => partsOf(new Date()));
  const [view, setView] = useState(() => ({ y: new Date().getFullYear(), m: new Date().getMonth() }));
  // 时/分草稿：正在键入时显示草稿，失焦/回车校验后写回（null = 显示 sel 里的值）
  const [hourDraft, setHourDraft] = useState<string | null>(null);
  const [minDraft, setMinDraft] = useState<string | null>(null);
  // 面板弹开方向：按钮上方空间够就向上（悬浮窗贴屏幕底边必须向上），否则向下（任务页）
  const rowRef = useRef<HTMLDivElement>(null);
  const [panelUp, setPanelUp] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const openPicker = () => {
    const base = dueLocal ? new Date(dueLocal) : new Date();
    if (!Number.isNaN(base.getTime())) {
      setSel(partsOf(base));
      setView({ y: base.getFullYear(), m: base.getMonth() });
    }
    const rect = rowRef.current?.getBoundingClientRect();
    setPanelUp(rect ? rect.top > 320 : true);
    setPickerOpen(true);
  };

  const setQuickDue = (days: number, hour: number, minute: number) => {
    const d = new Date();
    d.setDate(d.getDate() + days);
    d.setHours(hour, minute, 0, 0);
    setDueLocal(isoToLocalInput(d.toISOString()));
    setPickerOpen(false);
  };

  const confirmPicker = () => {
    const date = new Date(sel.y, sel.m, sel.d, sel.h, sel.min, 0, 0);
    setDueLocal(isoToLocalInput(date.toISOString()));
    setPickerOpen(false);
  };

  const submit = async () => {
    const trimmed = title.trim();
    if (!trimmed) return;
    const dueAt = localInputToIso(dueLocal);
    if (task) {
      await update(task.id, { title: trimmed, priority, dueAt });
    } else {
      await add({ title: trimmed, priority, dueAt });
    }
    setAdding(false);
    onDone();
  };

  // 月份视图数据
  const firstWeekday = new Date(view.y, view.m, 1).getDay();
  const daysInMonth = new Date(view.y, view.m + 1, 0).getDate();
  const today = new Date();
  const dueDate = dueLocal ? new Date(dueLocal) : null;
  const dueLabel = dueDate && !Number.isNaN(dueDate.getTime())
    ? `${dueDate.getMonth() + 1}/${dueDate.getDate()} ${pad(dueDate.getHours())}:${pad(dueDate.getMinutes())}`
    : null;

  const commitHour = () => {
    if (hourDraft === null) return;
    const n = Number(hourDraft);
    if (Number.isInteger(n) && n >= 0 && n <= 23) {
      setSel((s) => ({ ...s, h: n }));
    }
    setHourDraft(null);
  };
  const commitMinute = () => {
    if (minDraft === null) return;
    const n = Number(minDraft);
    if (Number.isInteger(n) && n >= 0 && n <= 59) {
      setSel((s) => ({ ...s, min: n }));
    }
    setMinDraft(null);
  };

  return (
    <div className="px-3 py-2" data-tauri-drag-region="false">
      <input
        ref={inputRef}
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') void submit();
          if (e.key === 'Escape') onDone();
        }}
        placeholder={task ? '修改任务…' : '要做什么？例如：明天 18:00 完成方案'}
        className="w-full rounded-md bg-black/5 px-2 py-1.5 text-sm outline-none placeholder:text-zinc-400 focus:bg-black/10 dark:bg-white/10 dark:focus:bg-white/15 dark:placeholder:text-zinc-500"
      />
      <div className="mt-1.5 flex flex-wrap items-center gap-1">
        {QUICK_DUE.map((q) => (
          <button
            key={q.label}
            type="button"
            onClick={() => setQuickDue(q.days, q.hour, q.minute)}
            className="rounded-full bg-black/5 px-2 py-0.5 text-[11px] text-zinc-600 transition hover:bg-blue-500 hover:text-white dark:bg-white/10 dark:text-zinc-300"
          >
            {q.label}
          </button>
        ))}
      </div>
      <div
        ref={rowRef}
        className="relative mt-1.5 flex items-center gap-2 text-xs"
      >
        <select
          value={priority}
          onChange={(e) => setPriority(e.target.value as Priority)}
          className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 outline-none dark:border-white/20"
        >
          {(Object.keys(PRIORITY_META) as Priority[]).map((p) => (
            <option key={p} value={p}>
              {PRIORITY_META[p].label}
            </option>
          ))}
        </select>

        {/* 时间选择按钮 + 自绘选择面板（锚定整行左缘，右侧不会被窗口裁掉） */}
        <button
          type="button"
          onClick={() => (pickerOpen ? setPickerOpen(false) : openPicker())}
          className={`rounded-md border px-1.5 py-1 tabular-nums outline-none transition ${
            pickerOpen
              ? 'border-blue-400 text-blue-600 dark:text-blue-400'
              : dueLabel
                ? 'border-black/10 text-zinc-700 dark:border-white/20 dark:text-zinc-200'
                : 'border-dashed border-black/20 text-zinc-400 dark:border-white/25'
          }`}
        >
          ⏰ {dueLabel ?? '选择时间'}
        </button>

        {pickerOpen && (
            <div
              className={`absolute left-0 z-20 w-[216px] rounded-xl bg-white p-1.5 shadow-xl ring-1 ring-black/10 dark:bg-zinc-800 dark:ring-white/15 ${
                panelUp ? 'bottom-full mb-1' : 'top-full mt-1'
              }`}
            >
              <div className="flex items-center justify-between px-0.5">
                <button
                  type="button"
                  onClick={() => setView((v) => (v.m === 0 ? { y: v.y - 1, m: 11 } : { ...v, m: v.m - 1 }))}
                  className="rounded px-1.5 text-zinc-400 hover:bg-black/5 dark:hover:bg-white/10"
                >
                  ‹
                </button>
                <span className="text-xs font-medium tabular-nums">
                  {view.y} 年 {view.m + 1} 月
                </span>
                <button
                  type="button"
                  onClick={() => setView((v) => (v.m === 11 ? { y: v.y + 1, m: 0 } : { ...v, m: v.m + 1 }))}
                  className="rounded px-1.5 text-zinc-400 hover:bg-black/5 dark:hover:bg-white/10"
                >
                  ›
                </button>
              </div>
              <div className="grid grid-cols-7 text-center text-[9px] text-zinc-400">
                {WEEKDAYS.map((w) => (
                  <span key={w}>{w}</span>
                ))}
              </div>
              <div className="grid grid-cols-7 gap-y-0.5">
                {Array.from({ length: firstWeekday }).map((_, i) => (
                  <span key={`blank-${i}`} />
                ))}
                {Array.from({ length: daysInMonth }).map((_, i) => {
                  const day = i + 1;
                  const selected = sel.y === view.y && sel.m === view.m && sel.d === day;
                  const isToday =
                    today.getFullYear() === view.y && today.getMonth() === view.m && today.getDate() === day;
                  return (
                    <button
                      key={day}
                      type="button"
                      onClick={() => setSel((s) => ({ ...s, y: view.y, m: view.m, d: day }))}
                      className={`h-5 w-5 justify-self-center rounded-full text-[10px] tabular-nums transition ${
                        selected
                          ? 'bg-blue-500 text-white'
                          : isToday
                            ? 'text-blue-600 ring-1 ring-blue-300 dark:text-blue-400'
                            : 'text-zinc-600 hover:bg-black/5 dark:text-zinc-300 dark:hover:bg-white/10'
                      }`}
                    >
                      {day}
                    </button>
                  );
                })}
              </div>
              <div className="mt-1 flex items-center justify-between gap-1 border-t border-black/5 pt-1 text-xs dark:border-white/10">
                <div className="flex items-center gap-0.5">
                  <input
                    value={hourDraft ?? String(sel.h)}
                    onChange={(e) => setHourDraft(e.target.value)}
                    onBlur={commitHour}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                    }}
                    inputMode="numeric"
                    title="时（0 ~ 23），直接键入"
                    className="h-5 w-8 rounded-md border border-black/10 bg-transparent text-center text-xs tabular-nums outline-none focus:border-blue-400 dark:border-white/15"
                  />
                  <span className="text-zinc-400">:</span>
                  <input
                    value={minDraft ?? String(sel.min)}
                    onChange={(e) => setMinDraft(e.target.value)}
                    onBlur={commitMinute}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                    }}
                    inputMode="numeric"
                    title="分（0 ~ 59），任意分钟"
                    className="h-5 w-8 rounded-md border border-black/10 bg-transparent text-center text-xs tabular-nums outline-none focus:border-blue-400 dark:border-white/15"
                  />
                </div>
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => {
                      setDueLocal('');
                      setPickerOpen(false);
                    }}
                    className="rounded px-1.5 py-0.5 text-[10px] text-zinc-400 transition hover:text-red-500"
                  >
                    清空
                  </button>
                  <button
                    type="button"
                    onClick={confirmPicker}
                    className="rounded-md bg-blue-500 px-2.5 py-0.5 text-[10px] text-white transition hover:bg-blue-600"
                  >
                    确定
                  </button>
                </div>
              </div>
            </div>
          )}

        <div className="ml-auto flex gap-1.5">
          <button
            onClick={onDone}
            className="rounded-md px-2 py-1 text-zinc-500 hover:bg-black/5 dark:hover:bg-white/10"
          >
            取消
          </button>
          <button
            onClick={() => void submit()}
            disabled={!title.trim()}
            className="rounded-md bg-blue-500 px-2.5 py-1 text-white transition hover:bg-blue-600 disabled:opacity-40"
          >
            {task ? '保存' : '添加'}
          </button>
        </div>
      </div>
    </div>
  );
}
