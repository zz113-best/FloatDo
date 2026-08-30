import { create } from 'zustand';
import type { Task, TaskInputPayload, TaskUpdatePayload } from '../types';
import { taskService } from '../services/taskService';

interface TaskState {
  tasks: Task[];
  loaded: boolean;
  error: string | null;
  load: (retry?: number) => Promise<void>;
  add: (input: TaskInputPayload) => Promise<void>;
  update: (id: number, patch: TaskUpdatePayload) => Promise<void>;
  toggleComplete: (task: Task) => Promise<void>;
  remove: (id: number) => Promise<void>;
  snooze: (task: Task) => Promise<void>;
  reorder: (orderedIds: number[]) => Promise<void>;
  setError: (message: string | null) => void;
}

let errorTimer: ReturnType<typeof setTimeout> | undefined;

export const useTaskStore = create<TaskState>((set, get) => ({
  tasks: [],
  loaded: false,
  error: null,

  async load(retry = 0) {
    try {
      const tasks = await taskService.list();
      set({ tasks, loaded: true, error: null });
    } catch (e) {
      // 应用刚启动时前端可能抢在后端注册完数据库状态之前发起调用（窗口比
      // setup 先创建），自动重试几次，避免一次竞争就让悬浮窗卡在报错态
      if (retry < 5) {
        setTimeout(() => void get().load(retry + 1), 600 * (retry + 1));
        return;
      }
      set({ error: `任务加载失败: ${String(e)}` });
    }
  },

  async add(input) {
    try {
      const task = await taskService.create(input);
      set({ tasks: [...get().tasks, task], error: null });
    } catch (e) {
      set({ error: `添加任务失败: ${String(e)}` });
    }
  },

  async update(id, patch) {
    try {
      const updated = await taskService.update(id, patch);
      set({
        tasks: get().tasks.map((t) => (t.id === id ? updated : t)),
        error: null,
      });
    } catch (e) {
      set({ error: `更新任务失败: ${String(e)}` });
    }
  },

  async toggleComplete(task) {
    if (task.status === 'COMPLETED') {
      await get().update(task.id, { status: 'TODO', completedAt: null });
    } else {
      await get().update(task.id, { status: 'COMPLETED' });
    }
  },

  /** 拖拽排序：先乐观更新本地顺序，再落库；失败回拉。 */
  async reorder(orderedIds) {
    const byId = new Map(get().tasks.map((t) => [t.id, t]));
    const reordered = orderedIds
      .map((id) => byId.get(id))
      .filter((t): t is Task => Boolean(t))
      .concat(get().tasks.filter((t) => !orderedIds.includes(t.id)));
    set({ tasks: reordered });
    try {
      await taskService.reorder(orderedIds);
    } catch (e) {
      set({ error: `排序失败: ${String(e)}` });
      await get().load();
    }
  },

  async remove(id) {
    try {
      await taskService.remove(id);
      set({ tasks: get().tasks.filter((t) => t.id !== id), error: null });
    } catch (e) {
      set({ error: `删除任务失败: ${String(e)}` });
    }
  },

  async snooze(task) {
    const { snoozeToTomorrow } = await import('../utils/time');
    await get().update(task.id, { dueAt: snoozeToTomorrow(task.dueAt) });
  },

  setError(message) {
    set({ error: message });
    if (errorTimer) clearTimeout(errorTimer);
    if (message) {
      errorTimer = setTimeout(() => set({ error: null }), 4000);
    }
  },
}));
