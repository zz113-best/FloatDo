import { create } from 'zustand';
import {
  focusService,
  type FocusPhase,
  type FocusState,
} from '../services/focusService';

interface FocusStoreState {
  phase: FocusPhase;
  /** 当前阶段结束时间（RFC3339），IDLE 为 null */
  endsAt: string | null;
  /** 进行中的会话（FOCUS 阶段有值） */
  session: FocusState['session'];
  workMinutes: number;
  breakMinutes: number;
  /** 今日完成的专注总秒数 */
  todaySeconds: number;
  loaded: boolean;
  /** 拉取后端最新专注状态（启动时 + 事件推送都可以调） */
  apply: (state: FocusState) => void;
  refresh: (retry?: number) => Promise<void>;
  start: (taskId?: number | null, minutes?: number) => Promise<void>;
  stop: () => Promise<void>;
}

export const useFocusStore = create<FocusStoreState>((set, get) => ({
  phase: 'IDLE',
  endsAt: null,
  session: null,
  workMinutes: 25,
  breakMinutes: 5,
  todaySeconds: 0,
  loaded: false,

  apply(state) {
    const cur = get();
    // 轮询每秒拉到的状态没变化时跳过，避免无谓的重渲染
    if (
      cur.loaded &&
      cur.phase === state.phase &&
      cur.endsAt === state.endsAt &&
      cur.session?.id === state.session?.id &&
      cur.todaySeconds === state.todaySeconds
    ) {
      return;
    }
    set({
      phase: state.phase,
      endsAt: state.endsAt,
      session: state.session,
      workMinutes: state.workMinutes,
      breakMinutes: state.breakMinutes,
      todaySeconds: state.todaySeconds,
      loaded: true,
    });
    syncPolling(state.phase);
  },

  async refresh(retry = 0) {
    try {
      get().apply(await focusService.getState());
    } catch {
      // 启动瞬间后端可能尚未就绪，稍候重试
      if (retry < 5) {
        setTimeout(() => void get().refresh(retry + 1), 600 * (retry + 1));
      }
    }
  },

  async start(taskId, minutes) {
    try {
      get().apply(await focusService.start(taskId, minutes));
    } catch (e) {
      console.error('开始专注失败:', e);
    }
  },

  async stop() {
    try {
      get().apply(await focusService.stop());
    } catch (e) {
      console.error('停止专注失败:', e);
    }
  },
}));

/** 把剩余秒数格式化为 mm:ss。 */
export function formatCountdown(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const m = Math.floor(s / 60);
  return `${String(m).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
}

// ---------------------------------------------------------------------------
// 轮询兜底：专注/休息进行中每秒向后端拉一次状态。
// 阶段切换的即时通知走 focus://changed 事件，但事件链路偶发不达时，
// 轮询保证最迟 1 秒内 UI 仍会切到下一阶段（桌宠气泡也依赖这里的 store）。
// ---------------------------------------------------------------------------
let pollTimer: ReturnType<typeof setInterval> | undefined;

function syncPolling(phase: FocusPhase): void {
  const needPolling = phase !== 'IDLE';
  if (needPolling && pollTimer === undefined) {
    pollTimer = setInterval(() => {
      void useFocusStore
        .getState()
        .refresh()
        .catch(() => undefined);
    }, 1000);
  } else if (!needPolling && pollTimer !== undefined) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
}
