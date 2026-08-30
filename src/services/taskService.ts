import { invoke } from '@tauri-apps/api/core';
import type { Task, TaskInputPayload, TaskPage, TaskQueryPayload, TaskUpdatePayload } from '../types';

/**
 * 任务数据访问层：前端所有任务操作都经过这里，组件不直接调用 invoke。
 */
export const taskService = {
  list(): Promise<Task[]> {
    return invoke<Task[]>('get_tasks');
  },
  create(input: TaskInputPayload): Promise<Task> {
    return invoke<Task>('create_task', { input });
  },
  update(id: number, patch: TaskUpdatePayload): Promise<Task> {
    return invoke<Task>('update_task', { id, patch });
  },
  remove(id: number): Promise<void> {
    return invoke('delete_task', { id });
  },
  /** 任务记录查询：关键词/完成/逾期/两组日期范围筛选 + 分页（统计页表格用）。 */
  search(query: TaskQueryPayload): Promise<TaskPage> {
    return invoke<TaskPage>('search_tasks', {
      query: {
        keyword: query.keyword ?? '',
        completed: query.completed ?? null,
        overdue: query.overdue ?? null,
        dueFrom: query.dueFrom ?? null,
        dueTo: query.dueTo ?? null,
        completedFrom: query.completedFrom ?? null,
        completedTo: query.completedTo ?? null,
        priority: query.priority ?? null,
        page: query.page ?? 1,
        pageSize: query.pageSize ?? 10,
      },
    });
  },
  /** 打开主面板并切到「任务」页签。 */
  openCenter(): Promise<void> {
    return invoke('open_tasks');
  },
  /** 拖拽排序：按给定 id 顺序重写 sort_order。 */
  reorder(orderedIds: number[]): Promise<void> {
    return invoke('reorder_tasks', { orderedIds });
  },
};
