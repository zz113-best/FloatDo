import { invoke } from '@tauri-apps/api/core';
import type { TaskQueryPayload } from '../types';

/**
 * 统计数据全部来自后端实时聚合（focus_sessions / tasks 表），
 * 前端只负责展示，不做任何数据运算。
 */

export interface FocusDayStat {
  /** 本地日期 YYYY-MM-DD */
  date: string;
  /** 当天完成的专注总秒数 */
  focusSeconds: number;
  /** 当天完整完成的专注轮数 */
  sessions: number;
}

export interface TaskDayStat {
  date: string;
  completed: number;
}

export interface TaskOverview {
  total: number;
  completed: number;
  pending: number;
  overdue: number;
  /** 已完成任务里「完成时间晚于截止时间」的数量 */
  completedLate: number;
}

export interface RecentTask {
  id: number;
  title: string;
  completedAt: string | null;
  dueAt: string | null;
  /** 完成时间晚于截止时间（逾期完成） */
  late: boolean;
}

export interface TaskFocusStat {
  taskId: number | null;
  /** 任务已删除时为 null（显示「未关联/已删除任务」） */
  title: string | null;
  focusSeconds: number;
  sessions: number;
}

export interface StatsReport {
  /** 实际统计范围（天） */
  days: number;
  focusDays: FocusDayStat[];
  taskDays: TaskDayStat[];
  focusTotalSeconds: number;
  focusTotalSessions: number;
  focusTodaySeconds: number;
  taskOverview: TaskOverview;
  /** 最近完成的任务明细 */
  recentTasks: RecentTask[];
  /** 范围内专注时长按任务拆分（时长降序） */
  focusByTask: TaskFocusStat[];
}

export const statsService = {
  async get(days?: number): Promise<StatsReport> {
    return invoke<StatsReport>('get_stats', { days: days ?? null });
  },
  async openWindow(): Promise<void> {
    await invoke('open_stats');
  },
  /** 按当前筛选条件导出任务记录 CSV，返回保存路径（用户取消为 null）。 */
  async exportTasksCsv(query: TaskQueryPayload): Promise<string | null> {
    return invoke<string | null>('export_tasks_csv', {
      query: {
        keyword: query.keyword ?? '',
        completed: query.completed ?? null,
        overdue: query.overdue ?? null,
        priority: query.priority ?? null,
        dueFrom: query.dueFrom ?? null,
        dueTo: query.dueTo ?? null,
        completedFrom: query.completedFrom ?? null,
        completedTo: query.completedTo ?? null,
        page: 1,
        pageSize: 10,
      },
    });
  },
};
