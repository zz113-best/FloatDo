import { invoke } from '@tauri-apps/api/core';

/**
 * AI 对话：接口配置（OpenAI 兼容）存本机 SQLite，API Key 不回传前端。
 * 对话历史由聊天窗口自己持有，每次请求带上最近若干条。
 */

export interface AiConfig {
  baseUrl: string;
  model: string;
  /** 是否已配置 API Key */
  hasApiKey: boolean;
}

export interface AiChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

/** 桌宠窗口收到后把 AI 的最新回复弹成气泡（payload 为截断后的纯文本）。 */
export const AI_REPLY_EVENT = 'ai://reply';

export const aiService = {
  async getConfig(): Promise<AiConfig> {
    return invoke<AiConfig>('get_ai_config');
  },
  /** api_key 传空串表示保持不变。 */
  async setConfig(baseUrl: string, apiKey: string, model: string): Promise<void> {
    await invoke('set_ai_config', { baseUrl, apiKey, model });
  },
  /** 发一条测试消息验证配置是否可用，成功返回模型回复。 */
  async test(): Promise<string> {
    return invoke<string>('test_ai');
  },
  async chat(messages: AiChatMessage[]): Promise<string> {
    return invoke<string>('ai_chat', { messages });
  },
  async openChatWindow(): Promise<void> {
    // 主面板切到 AI 对话页签（面板窗口由后端显示）
    await invoke('open_chat');
  },
};
