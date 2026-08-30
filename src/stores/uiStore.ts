import { create } from 'zustand';

interface UiState {
  expanded: boolean;
  /** 正在编辑的任务 id；null 表示没有编辑 */
  editingId: number | null;
  adding: boolean;
  setExpanded: (value: boolean) => void;
  setEditingId: (id: number | null) => void;
  setAdding: (value: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  expanded: false,
  editingId: null,
  adding: false,
  setExpanded: (value) => set({ expanded: value }),
  setEditingId: (id) => set({ editingId: id }),
  setAdding: (value) => set({ adding: value }),
}));
