import { invoke } from '@tauri-apps/api/core';

/** 专注模式事件（Rust → widget / pet 窗口），任何阶段切换都会推送完整状态。 */
export const FOCUS_CHANGED_EVENT = 'focus://changed';

export type FocusPhase = 'IDLE' | 'FOCUS' | 'BREAK';

export interface FocusSession {
  id: number;
  taskId: number | null;
  startedAt: string;
  endedAt: string | null;
  plannedMinutes: number;
  actualSeconds: number;
  status: 'RUNNING' | 'COMPLETED' | 'INTERRUPTED';
}

export interface FocusState {
  phase: FocusPhase;
  endsAt: string | null;
  session: FocusSession | null;
  workMinutes: number;
  breakMinutes: number;
  /** 今日（本地零点起）完成的专注总秒数 */
  todaySeconds: number;
}

/**
 * 专注模式数据访问层：专注相关的 invoke / 事件名都集中在这里。
 * 计时权威在后端，前端只负责展示倒计时和发起开始/停止。
 */
export const focusService = {
  getState(): Promise<FocusState> {
    return invoke<FocusState>('get_focus_state');
  },
  /** 开始一轮专注；taskId 可不绑定任务，minutes 不传则用设置里的时长。 */
  start(taskId?: number | null, minutes?: number): Promise<FocusState> {
    return invoke<FocusState>('start_focus', {
      taskId: taskId ?? null,
      minutes: minutes ?? null,
    });
  },
  stop(): Promise<FocusState> {
    return invoke<FocusState>('stop_focus');
  },
};
