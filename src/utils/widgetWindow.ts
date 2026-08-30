import { getCurrentWindow, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import type { PhysicalPosition as PhysicalPositionType } from '@tauri-apps/api/dpi';

export const WIDGET_WIDTH = 300;
export const WIDGET_COLLAPSED_HEIGHT = 64;

/**
 * 调整悬浮窗尺寸，并保持窗口底边位置不变（窗口向上生长）。
 * 必须让窗口尺寸与卡片内容一致，否则透明区域会挡住桌面点击、鼠标移出也不会触发。
 */
export async function resizeWidget(height: number): Promise<void> {
  const win = getCurrentWindow();
  try {
    const [pos, size, factor] = await Promise.all([
      win.outerPosition(),
      win.outerSize(),
      win.scaleFactor(),
    ]);
    const bottom = pos.y + size.height;
    await win.setSize(new LogicalSize(WIDGET_WIDTH, height));
    const newHeightPhysical = Math.round(height * factor);
    await win.setPosition(new PhysicalPosition(pos.x, bottom - newHeightPhysical) as PhysicalPositionType);
  } catch (e) {
    console.error('调整悬浮窗尺寸失败:', e);
  }
}
