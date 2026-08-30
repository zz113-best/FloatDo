import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { aiService, type AiChatMessage } from '../../services/aiService';
import {
  petService,
  PET_PERSONALITIES,
  PET_PERSONALITY_CHANGED_EVENT,
  type PetPersonalityId,
} from '../../services/petService';
import { useSettingsStore } from '../../stores/settingsStore';

const GREETING =
  '喵~ 我是你的桌宠小猫！问我今天的任务、要不要开始专注，或者随便聊聊天都行。';

/** 各人格在对话页头部显示的名字（与桌宠中心、气泡文案同一套人设）。 */
const PERSONALITY_NAMES: Record<PetPersonalityId, string> = {
  gentle: '温柔小猫',
  motivator: '热血教练猫',
  sarcastic: '毒舌小猫',
  cool: '高冷小猫',
};

interface ChatItem extends AiChatMessage {
  /** 请求失败时展示的错误行（只做展示，不进入对话历史） */
  error?: boolean;
}

/** AI 聊天窗口页面：桌宠人设对话，后端自动注入真实任务/专注上下文。 */
export function ChatPage() {
  const { load: loadSettings } = useSettingsStore();
  const [items, setItems] = useState<ChatItem[]>([
    { role: 'assistant', content: GREETING },
  ]);
  const [draft, setDraft] = useState('');
  const [sending, setSending] = useState(false);
  const [aiReady, setAiReady] = useState<boolean | null>(null);
  const [personality, setPersonality] = useState<PetPersonalityId>('gentle');
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void loadSettings();
    void aiService
      .getConfig()
      .then((c) => setAiReady(c.hasApiKey && c.baseUrl.trim() !== '' && c.model.trim() !== ''));
    void petService
      .getPersonality()
      .then(setPersonality)
      .catch(() => undefined);
    const unlistenPersonality = listen<string>(PET_PERSONALITY_CHANGED_EVENT, (e) => {
      setPersonality(e.payload as PetPersonalityId);
    });
    return () => {
      unlistenPersonality.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 新消息自动滚到底
  useEffect(() => {
    listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
  }, [items, sending]);

  const send = async () => {
    const text = draft.trim();
    if (!text || sending) return;
    setDraft('');
    const history = items.filter((m) => !m.error);
    const next: ChatItem[] = [...history, { role: 'user', content: text }];
    setItems([...next, { role: 'assistant', content: '', error: false }]);
    setSending(true);
    try {
      // 发给后端的是不含错误行、不含本地占位回复的历史
      const payload: AiChatMessage[] = [...history, { role: 'user', content: text }];
      const reply = await aiService.chat(payload);
      setItems([...next, { role: 'assistant', content: reply }]);
    } catch (e) {
      setItems([...next, { role: 'assistant', content: String(e), error: true }]);
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="flex h-full flex-col bg-zinc-100 text-zinc-800 dark:bg-zinc-900 dark:text-zinc-200">
      <header className="flex items-center gap-2 border-b border-black/10 bg-white px-4 py-3 dark:border-white/10 dark:bg-zinc-800">
        <span aria-hidden className="text-lg">🐱</span>
        <div>
          <div className="text-sm font-medium">
            {PERSONALITY_NAMES[personality]}
            <span className="ml-2 text-xs font-normal text-zinc-400">
              {PET_PERSONALITIES.find((p) => p.id === personality)?.label}人格
            </span>
          </div>
          <div className="text-xs text-zinc-400">
            {aiReady === false ? '未配置 AI 接口' : '了解你的任务和专注进度'}
          </div>
        </div>
      </header>

      <div ref={listRef} className="flex-1 space-y-3 overflow-y-auto p-4">
        {items.map((m, i) => (
          <div
            key={i}
            className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'}`}
          >
            <div
              className={`max-w-[80%] whitespace-pre-wrap break-words rounded-2xl px-3 py-2 text-sm leading-relaxed ${
                m.error
                  ? 'bg-red-50 text-red-500 ring-1 ring-red-200 dark:bg-red-500/10 dark:ring-red-500/30'
                  : m.role === 'user'
                    ? 'rounded-br-sm bg-blue-500 text-white'
                    : 'rounded-bl-sm bg-white text-zinc-700 shadow-sm ring-1 ring-black/5 dark:bg-zinc-800 dark:text-zinc-200 dark:ring-white/10'
              }`}
            >
              {m.content ||
                (sending && i === items.length - 1 ? '思考中…' : '')}
            </div>
          </div>
        ))}
        {aiReady === false && (
          <div className="rounded-xl bg-amber-50 px-3 py-2 text-xs leading-relaxed text-amber-600 ring-1 ring-amber-200 dark:bg-amber-500/10 dark:text-amber-400 dark:ring-amber-500/30">
            还没有配置 AI 接口。到「设置 → AI 助手」填写 OpenAI 兼容的接口地址、
            API Key 和模型名后就能聊天，Key 只保存在本机。
          </div>
        )}
      </div>

      <footer className="border-t border-black/10 bg-white p-3 dark:border-white/10 dark:bg-zinc-800">
        <div className="flex items-end gap-2">
          <textarea
            value={draft}
            rows={2}
            placeholder="和小猫聊聊…（Enter 发送，Shift+Enter 换行）"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
            className="min-w-0 flex-1 resize-none rounded-lg border border-black/10 px-3 py-2 text-sm outline-none focus:border-blue-400 dark:border-white/10 dark:bg-zinc-700"
          />
          <button
            type="button"
            onClick={() => void send()}
            disabled={sending || !draft.trim()}
            className="rounded-lg bg-blue-500 px-4 py-2 text-sm text-white transition hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            发送
          </button>
        </div>
      </footer>
    </div>
  );
}
