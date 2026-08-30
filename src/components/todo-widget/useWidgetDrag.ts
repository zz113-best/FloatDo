import { useCallback } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * 悬浮窗拖动：不依赖 data-tauri-drag-region（它要求 mousedown 命中元素本身，
 * 点到文字子元素就失效），改为显式调用 startDragging。
 * 按钮/输入框等交互元素上的按下不触发拖动。
 */
export function useWidgetDrag(): (e: React.MouseEvent) => void {
  return useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest('button, input, select, textarea, [role="listbox"]')) return;
    // 任务行保留给「拖拽排序任务」，不作为窗口拖动热区
    if (target.closest('[data-task-drag]')) return;
    e.preventDefault();
    void getCurrentWindow().startDragging();
  }, []);
}
