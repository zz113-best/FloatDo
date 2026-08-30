export type TaskStatus = 'TODO' | 'IN_PROGRESS' | 'COMPLETED' | 'OVERDUE' | 'CANCELLED';
export type Priority = 'LOW' | 'MEDIUM' | 'HIGH' | 'URGENT';

export interface Task {
  id: number;
  title: string;
  description: string;
  status: TaskStatus;
  priority: Priority;
  categoryId: number | null;
  tags: string;
  createdAt: string;
  updatedAt: string;
  dueAt: string | null;
  completedAt: string | null;
  estimatedMinutes: number | null;
  reminderEnabled: boolean;
  reminderTime: string | null;
  repeatRule: string | null;
  sortOrder: number;
}

export interface TaskInputPayload {
  title: string;
  description?: string;
  priority?: Priority;
  categoryId?: number | null;
  dueAt?: string | null;
  estimatedMinutes?: number | null;
}

export interface TaskUpdatePayload {
  title?: string;
  description?: string;
  status?: TaskStatus;
  priority?: Priority;
  categoryId?: number | null;
  dueAt?: string | null;
  completedAt?: string | null;
  estimatedMinutes?: number | null;
  reminderEnabled?: boolean;
  reminderTime?: string | null;
  repeatRule?: string | null;
  sortOrder?: number;
}

export interface Category {
  id: number;
  name: string;
  icon: string;
  isDefault: boolean;
  sortOrder: number;
}

export type ThemeMode = 'light' | 'dark' | 'system';

/** 任务记录查询参数（统计页表格）。 */
export interface TaskQueryPayload {
  keyword?: string;
  /** true=已完成 false=未完成 null/undefined=全部 */
  completed?: boolean | null;
  /** true=逾期（未完成且截止已过）false=未逾期 */
  overdue?: boolean | null;
  /** 截止日期范围，YYYY-MM-DD 本地日期 */
  dueFrom?: string | null;
  dueTo?: string | null;
  /** 完成日期范围，YYYY-MM-DD 本地日期 */
  completedFrom?: string | null;
  completedTo?: string | null;
  /** 按优先级筛选：URGENT/HIGH/MEDIUM/LOW，空 = 全部 */
  priority?: string | null;
  page?: number;
  pageSize?: number;
}

export interface TaskPage {
  items: Task[];
  total: number;
  page: number;
  pageSize: number;
}
