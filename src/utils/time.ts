/** 截止时间相关的格式化与判断。所有函数接受 RFC3339 字符串。 */

export function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

export function isToday(iso: string | null): boolean {
  if (!iso) return false;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return false;
  return isSameDay(d, new Date());
}

export function isOverdue(iso: string | null): boolean {
  if (!iso) return false;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return false;
  return d.getTime() < Date.now();
}

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

/** 截止时间的紧凑展示：今天只显示时间，过去显示“逾期 x”，其余显示日期。 */
export function formatDue(iso: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const now = new Date();
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;

  if (d.getTime() < now.getTime()) {
    const diffMs = now.getTime() - d.getTime();
    const minutes = Math.floor(diffMs / 60000);
    if (minutes < 60) return `逾期 ${minutes} 分钟`;
    const hours = Math.floor(minutes / 60);
    if (isSameDay(d, now)) return `逾期 ${hours} 小时`;
    const days = Math.floor(hours / 24);
    return days === 0 ? `逾期 ${hours} 小时` : `逾期 ${days} 天`;
  }

  if (isSameDay(d, now)) return time;
  const tomorrow = new Date(now);
  tomorrow.setDate(now.getDate() + 1);
  if (isSameDay(d, tomorrow)) return `明天 ${time}`;
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${time}`;
}

/** 把 datetime-local 输入框的值转成 RFC3339；空值返回 null。 */
export function localInputToIso(value: string): string | null {
  if (!value) return null;
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString();
}

/** RFC3339 → datetime-local 输入框格式（本地时区）。 */
export function isoToLocalInput(iso: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** “推迟到明天”：保留原来的时间，日期 +1 天。无截止时间则设为明天同一时刻。 */
export function snoozeToTomorrow(iso: string | null): string {
  const base = iso ? new Date(iso) : new Date();
  base.setDate(base.getDate() + 1);
  return base.toISOString();
}
