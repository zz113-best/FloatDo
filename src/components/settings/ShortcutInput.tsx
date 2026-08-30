import { useState } from 'react';
import { formatCombo } from '../../services/shortcutService';

const MODIFIER_KEYS = new Set(['Control', 'Alt', 'Shift', 'Meta']);

/** e.key → 存储键名。只接受后端 global-hotkey 解析器明确支持的键。 */
function keyToken(key: string): string | null {
  if (/^[a-z]$/i.test(key)) return key.toUpperCase();
  if (/^[0-9]$/.test(key)) return key;
  if (/^F([1-9]|1[0-9]|2[0-4])$/i.test(key)) return key.toUpperCase();
  const map: Record<string, string> = {
    ' ': 'Space',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right',
    Enter: 'Enter',
    '-': 'Minus',
    '=': 'Equal',
    ',': 'Comma',
    '.': 'Period',
    '`': 'Backquote',
    '\\': 'Backslash',
    '[': 'BracketLeft',
    ']': 'BracketRight',
  };
  return map[key] ?? null;
}

/**
 * 快捷键录制按钮：点击进入录制态，按下组合键即提交。
 * Esc 取消录制；Backspace / Delete 清除（= 停用该快捷键）。
 * 至少要带一个修饰键，避免裸键劫持系统里的正常打字。
 */
export function ShortcutInput({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (value: string) => Promise<void>;
}) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleKeyDown = async (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      setRecording(false);
      return;
    }
    if (MODIFIER_KEYS.has(e.key)) {
      return; // 只按了修饰键，等主键
    }
    if (e.key === 'Backspace' || e.key === 'Delete') {
      setRecording(false);
      setError(null);
      try {
        await onCommit('');
      } catch (err) {
        setError(String(err));
      }
      return;
    }
    // 单独按空格 = 清除停用（带修饰键的空格才是录制 Space 键）
    if (e.key === ' ' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
      setRecording(false);
      setError(null);
      try {
        await onCommit('');
      } catch (err) {
        setError(String(err));
      }
      return;
    }
    const token = keyToken(e.key);
    if (!token) {
      setError('仅支持 字母 / 数字 / F1~F24 / 方向键 / 空格 等常用键');
      return;
    }
    const parts = [
      e.ctrlKey && 'Ctrl',
      e.altKey && 'Alt',
      e.shiftKey && 'Shift',
      e.metaKey && 'Super',
    ].filter(Boolean) as string[];
    if (parts.length === 0) {
      setError('至少要包含一个修饰键（Ctrl / Alt / Shift）· 单独按空格可清除');
      return;
    }
    setRecording(false);
    try {
      setError(null);
      await onCommit([...parts, token].join('+'));
    } catch (err) {
      // 后端注册失败（多为组合键被其他程序占用），文案直接展示
      setError(String(err));
    }
  };

  return (
    <div>
      <button
        type="button"
        tabIndex={0}
        onClick={() => {
          setRecording(true);
          setError(null);
        }}
        onKeyDown={(e) => void handleKeyDown(e)}
        onBlur={() => setRecording(false)}
        className={`min-w-40 rounded-md border px-3 py-1.5 text-sm tabular-nums transition ${
          recording
            ? 'border-blue-400 text-blue-500'
            : 'border-black/10 hover:border-blue-300 dark:border-white/10 dark:hover:border-blue-500/50'
        }`}
      >
        {recording ? '按下组合键…' : value ? formatCombo(value) : '未设置（点击录制）'}
      </button>
      {recording && (
        <p className="mt-1 text-xs text-zinc-400">
          Esc 取消 · 空格 / Backspace 清除停用
        </p>
      )}
      {error && <p className="mt-1 text-xs text-red-500">{error}</p>}
    </div>
  );
}
