import { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSettingsStore } from '../../stores/settingsStore';
import { SETTINGS_CHANGED_EVENT } from '../../services/settingsService';
import {
  statsService,
  type FocusDayStat,
  type RecentTask,
  type TaskFocusStat,
} from '../../services/statsService';
import { TaskRecords } from './TaskRecords';

const RANGE_OPTIONS = [7, 30] as const;

/**
 * 统计页（主面板的一个页签）：专注时长 + 任务完成情况。
 * 数据全部来自后端实时聚合；MainShell 在每次打开面板时会以新 key
 * 重挂载本页，所以每次进来都拉的是最新数据。
 */
export function StatsPage() {
  const { load: loadSettings } = useSettingsStore();
  const [days, setDays] = useState<number>(7);
  const [report, setReport] = useState<Awaited<ReturnType<typeof statsService.get>> | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = (range: number) => {
    void statsService
      .get(range)
      .then((r) => {
        setReport(r);
        setError(null);
      })
      .catch((e) => setError(`加载统计数据失败: ${String(e)}`));
  };

  useEffect(() => {
    void loadSettings();
    load(days);
    // 设置窗口改了主题时同步
    const unsettings = listen(SETTINGS_CHANGED_EVENT, () => {
      void loadSettings();
    });
    return () => {
      unsettings.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 切换范围重新拉取
  useEffect(() => {
    load(days);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [days]);

  if (error) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-red-500">
        {error}
      </div>
    );
  }
  if (!report) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-zinc-400">
        加载中…
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mx-auto max-w-4xl space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-lg font-semibold leading-tight">统计</h1>
            <p className="mt-0.5 text-xs text-zinc-400">你的专注与任务数据，全部来自本机</p>
          </div>
          <div className="flex rounded-lg bg-black/5 p-0.5 dark:bg-white/10">
            {RANGE_OPTIONS.map((r) => (
              <button
                key={r}
                onClick={() => setDays(r)}
                className={`rounded-md px-3 py-1 text-sm transition ${
                  days === r
                    ? 'bg-white text-blue-600 shadow-sm dark:bg-zinc-700 dark:text-blue-400'
                    : 'text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200'
                }`}
              >
                近 {r} 天
              </button>
            ))}
          </div>
        </div>

        {/* 概览卡片：一行四个 */}
        <div className="grid grid-cols-4 gap-3">
          <StatCard
            icon="🎯"
            tone="blue"
            label="今日专注"
            value={formatMinutes(report.focusTodaySeconds)}
            sub={report.focusTodaySeconds > 0 ? `完成 ${countTodaySessions(report.focusDays)} 轮` : '今天还没有专注记录'}
          />
          <StatCard
            icon="📊"
            tone="indigo"
            label={`近 ${report.days} 天专注`}
            value={formatMinutes(report.focusTotalSeconds)}
            sub={`共 ${report.focusTotalSessions} 轮完整专注`}
          />
          <StatCard
            icon="✅"
            tone="emerald"
            label="任务完成"
            value={`${report.taskOverview.completed} / ${report.taskOverview.total}`}
            sub={`待办 ${report.taskOverview.pending} 个`}
          />
          <StatCard
            icon="⏰"
            tone="red"
            label="逾期任务"
            value={`${report.taskOverview.overdue} 个`}
            sub={
              report.taskOverview.overdue > 0
                ? '先从逾期任务开始吧'
                : report.taskOverview.completedLate > 0
                  ? `另有 ${report.taskOverview.completedLate} 个逾期后才完成`
                  : '没有逾期，保持住！'
            }
          />
        </div>

        {/* 双栏图表 */}
        <div className="grid grid-cols-2 items-start gap-4">
          <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
            <h2 className="mb-3 flex items-baseline justify-between text-sm font-medium">
              专注时长
              <span className="text-xs font-normal text-zinc-400">分钟 / 天</span>
            </h2>
            <DayBars
              data={report.focusDays.map((d) => ({ date: d.date, value: Math.round(d.focusSeconds / 60), extra: d.sessions }))}
              colorClass="bg-blue-500"
              labelEvery={days > 7 ? 5 : 1}
              formatTooltip={(date, minutes, sessions) =>
                `${date}：专注 ${formatMinutes(minutes * 60)}（${sessions} 轮）`
              }
            />
          </section>

          <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
            <h2 className="mb-3 flex items-baseline justify-between text-sm font-medium">
              完成任务
              <span className="text-xs font-normal text-zinc-400">个数 / 天</span>
            </h2>
            <DayBars
              data={report.taskDays.map((d) => ({ date: d.date, value: d.completed, extra: d.completed }))}
              colorClass="bg-emerald-500"
              labelEvery={days > 7 ? 5 : 1}
              formatTooltip={(date, count) => `${date}：完成 ${count} 个任务`}
            />
          </section>
        </div>

        {/* 最近完成：横向卡片流，多的时候左右滑动 */}
        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 flex items-baseline justify-between text-sm font-medium">
            最近完成
            <span className="text-xs font-normal text-zinc-400">{report.recentTasks.length} 条</span>
          </h2>
          {report.recentTasks.length === 0 ? (
            <p className="text-xs text-zinc-400">还没有完成的任务，去悬浮窗勾一个吧。</p>
          ) : (
            <div className="flex gap-2 overflow-x-auto pb-1">
              {report.recentTasks.map((t) => (
                <RecentTaskCard key={t.id} task={t} />
              ))}
            </div>
          )}
        </section>

        {/* 专注分布：时间花在了哪些任务上 */}
        <FocusByTask items={report.focusByTask} days={days} />

        {/* 全部任务记录：占满整行 */}
        <TaskRecords />

        <p className="text-xs leading-relaxed text-zinc-400">
          数据实时读取本机 SQLite 数据库（focus_sessions / tasks 表），关闭窗口后再次打开会自动刷新。
        </p>
      </div>
    </div>
  );
}

/** 专注时长按任务拆分：横向条形，一眼看出时间花在哪。 */
function FocusByTask({ items, days }: { items: TaskFocusStat[]; days: number }) {
  const withTime = items.filter((i) => i.focusSeconds > 0);
  const max = Math.max(...withTime.map((i) => i.focusSeconds), 1);
  return (
    <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
      <h2 className="mb-3 flex items-baseline justify-between text-sm font-medium">
        专注分布
        <span className="text-xs font-normal text-zinc-400">近 {days} 天 · 按任务</span>
      </h2>
      {withTime.length === 0 ? (
        <p className="text-xs text-zinc-400">
          这段时间还没有专注记录。开始专注时关联任务，就能看到时间花在了哪里。
        </p>
      ) : (
        <ul className="space-y-2">
          {withTime.map((i) => (
            <li key={i.taskId ?? 'none'} className="flex items-center gap-2 text-sm">
              <span className="w-36 truncate text-zinc-700 dark:text-zinc-200" title={i.title ?? undefined}>
                {i.title ?? '未关联 / 已删除任务'}
              </span>
              <div className="h-2.5 min-w-0 flex-1 overflow-hidden rounded-full bg-black/5 dark:bg-white/10">
                <div
                  className="h-full rounded-full bg-indigo-500"
                  style={{ width: `${Math.max(4, Math.round((i.focusSeconds / max) * 100))}%` }}
                />
              </div>
              <span className="w-24 shrink-0 text-right text-xs tabular-nums text-zinc-400">
                {formatMinutes(i.focusSeconds)} · {i.sessions} 轮
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** 最近完成的横向卡片：任务名 + 完成时间 + 截止时间 + 逾期标记。 */
function RecentTaskCard({ task }: { task: RecentTask }) {
  return (
    <div className="w-44 shrink-0 rounded-lg bg-black/[0.03] p-2.5 dark:bg-white/5">
      <div className="flex items-start gap-1.5">
        <span className="mt-0.5 shrink-0 text-emerald-500" aria-hidden>
          ✓
        </span>
        <span className="line-clamp-2 break-all text-sm leading-snug">{task.title}</span>
      </div>
      <div className="mt-1.5 space-y-0.5 text-xs text-zinc-400">
        <div>完成于 {formatDateTime(task.completedAt)}</div>
        {task.dueAt && <div>截止 {formatDateTime(task.dueAt)}</div>}
        {task.late && (
          <span className="inline-block rounded bg-red-100 px-1 text-red-500 dark:bg-red-500/15">
            逾期完成
          </span>
        )}
      </div>
    </div>
  );
}

/** RFC3339 → 本地「M/D HH:mm」，空值或非法值显示「—」。 */
function formatDateTime(iso: string | null): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '—';
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const TONES = {
  blue: 'bg-blue-500/10 text-blue-500',
  indigo: 'bg-indigo-500/10 text-indigo-500',
  emerald: 'bg-emerald-500/10 text-emerald-500',
  red: 'bg-red-500/10 text-red-500',
} as const;

function StatCard({
  icon,
  tone,
  label,
  value,
  sub,
}: {
  icon: string;
  tone: keyof typeof TONES;
  label: string;
  value: string;
  sub: string;
}) {
  return (
    <div className="rounded-xl bg-white p-4 shadow-sm transition hover:shadow-md dark:bg-zinc-800">
      <div className="flex items-center gap-2">
        <span
          className={`flex h-7 w-7 items-center justify-center rounded-lg text-sm ${TONES[tone]}`}
          aria-hidden
        >
          {icon}
        </span>
        <span className="truncate text-xs text-zinc-400">{label}</span>
      </div>
      <div className="mt-2 text-xl font-semibold tabular-nums">{value}</div>
      <div className="mt-0.5 truncate text-xs text-zinc-400" title={sub}>
        {sub}
      </div>
    </div>
  );
}

function DayBars({
  data,
  colorClass,
  labelEvery,
  formatTooltip,
}: {
  data: { date: string; value: number; extra: number }[];
  colorClass: string;
  /** 每 N 天显示一个日期标签（30 天时避免挤在一起） */
  labelEvery: number;
  formatTooltip: (date: string, value: number, extra: number) => string;
}) {
  const max = useMemo(() => Math.max(1, ...data.map((d) => d.value)), [data]);
  const lastIndex = data.length - 1;

  return (
    <div>
      <div className="flex h-32 items-end gap-[3px]">
        {data.map((d, i) => {
          const isToday = i === lastIndex;
          return (
            <div
              key={d.date}
              title={formatTooltip(d.date, d.value, d.extra)}
              className="group flex h-full flex-1 cursor-default flex-col justify-end"
            >
              <div
                className={`${colorClass} rounded-t-sm transition-all group-hover:opacity-80 ${
                  d.value === 0 ? 'opacity-20' : ''
                } ${isToday ? 'ring-1 ring-blue-400/60 ring-offset-0' : ''}`}
                style={{ height: d.value === 0 ? 3 : `${Math.max((d.value / max) * 100, 4)}%` }}
              />
            </div>
          );
        })}
      </div>
      <div className="mt-1 flex gap-[3px] text-[10px] text-zinc-400">
        {data.map((d, i) => (
          <div key={d.date} className="flex-1 text-center">
            {i % labelEvery === 0 || i === lastIndex ? shortDate(d.date) : ''}
          </div>
        ))}
      </div>
    </div>
  );
}

/** 秒 → 中文时长：不足 1 小时显示分钟，否则「X 小时 Y 分」。 */
function formatMinutes(seconds: number): string {
  const m = Math.round(seconds / 60);
  if (m < 1) return '0 分钟';
  if (m < 60) return `${m} 分钟`;
  const h = Math.floor(m / 60);
  const rest = m % 60;
  return rest === 0 ? `${h} 小时` : `${h} 小时 ${rest} 分`;
}

function countTodaySessions(focusDays: FocusDayStat[]): number {
  return focusDays.length > 0 ? focusDays[focusDays.length - 1].sessions : 0;
}

/** YYYY-MM-DD → M/D */
function shortDate(date: string): string {
  const [, m, d] = date.split('-');
  const day = Number(d);
  const today = new Date();
  if (Number(m) === today.getMonth() + 1 && day === today.getDate()) return '今天';
  return `${Number(m)}/${day}`;
}
