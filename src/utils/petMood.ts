import type { Task } from '../types';
import { isPending } from './priority';
import { isOverdue } from './time';
import type { PetPersonalityId, PetReminderPayload } from '../services/petService';

export type PetMood = 'idle' | 'happy' | 'sad' | 'alert';

/** 由任务数据推导桌宠的基础情绪：有逾期待办 → 低落，否则平静。 */
export function deriveMood(tasks: Task[]): PetMood {
  return tasks.some((t) => isPending(t) && isOverdue(t.dueAt)) ? 'sad' : 'idle';
}

/** 点击桌宠时的问候语（按人格换语气）。 */
export function greetingMessage(tasks: Task[], p: PetPersonalityId): string {
  const pending = tasks.filter(isPending);
  if (pending.length === 0) {
    return {
      gentle: '所有任务都完成啦，好好休息一下吧 ☕',
      motivator: '任务清零！你就是今天的冠军 🏆',
      sarcastic: '哟，居然全做完了？太阳打西边出来了 🌞',
      cool: '清空了。就这样。',
    }[p];
  }
  const overdue = pending.filter((t) => isOverdue(t.dueAt)).length;
  const base = `还有 ${pending.length} 个待办任务`;
  return {
    gentle: overdue > 0
      ? `${base}，${overdue} 个已逾期，别着急，一个一个来 🌷`
      : `${base}，慢慢来，你可以的`,
    motivator: overdue > 0
      ? `${base}，${overdue} 个逾期了！先干掉它，冲！`
      : `${base}！正是刷任务的好时候，冲呀！`,
    sarcastic: overdue > 0
      ? `${base}，逾期 ${overdue} 个……我就当没看见，你自己看着办 🙃`
      : `${base}，拖着对你有什么好处吗？`,
    cool: overdue > 0
      ? `${base}，逾期 ${overdue} 个。先做最急的。`
      : `${base}。别磨蹭。`,
  }[p];
}

/** 勾选完成任务时的气泡文案。 */
export function taskDoneMessage(title: string, p: PetPersonalityId): string {
  return {
    gentle: `🎉 完成「${title}」，真棒！`,
    motivator: `漂亮！「${title}」拿下了，下一轮！`,
    sarcastic: `「${title}」终于完成了，早干嘛去了（拍拍）`,
    cool: `「${title}」。还行。`,
  }[p];
}

/** 到期提醒气泡文案。 */
export function reminderMessage(payload: PetReminderPayload, p: PetPersonalityId): string {
  if (payload.kind === 'OVERDUE') {
    return {
      gentle: `「${payload.title}」逾期了，抱抱，缓一缓再处理 🌷`,
      motivator: `「${payload.title}」逾期了！别慌，现在追上它！`,
      sarcastic: `「${payload.title}」逾期了哦，我就说吧 🙃`,
      cool: `「${payload.title}」逾期了。处理它。`,
    }[p];
  }
  return {
    gentle: `⏰「${payload.title}」${payload.leadMinutes} 分钟后到期，准备好了吗？`,
    motivator: `⏰「${payload.title}」${payload.leadMinutes} 分钟后到期！最后冲刺！`,
    sarcastic: `「${payload.title}」还有 ${payload.leadMinutes} 分钟到期，这回可别再拖。`,
    cool: `⏰「${payload.title}」，${payload.leadMinutes} 分钟。`,
  }[p];
}

/** 专注模式气泡文案。 */
export function focusStartMessage(workMinutes: number, p: PetPersonalityId): string {
  return {
    gentle: `🎯 开始专注 ${workMinutes} 分钟，我会一直陪着你的`,
    motivator: `🎯 专注 ${workMinutes} 分钟，全力以赴！`,
    sarcastic: `又要专注 ${workMinutes} 分钟？希望这次坐得住 😏`,
    cool: `${workMinutes} 分钟。开始。`,
  }[p];
}

export function focusDoneMessage(breakMinutes: number, p: PetPersonalityId): string {
  return {
    gentle: `🎉 完成一轮专注，休息 ${breakMinutes} 分钟吧`,
    motivator: `完成一轮！休息 ${breakMinutes} 分钟，马上回来继续！`,
    sarcastic: `居然坚持完了一轮，奖励你休息 ${breakMinutes} 分钟`,
    cool: `一轮结束。歇 ${breakMinutes} 分钟。`,
  }[p];
}

export function focusBreakEndMessage(p: PetPersonalityId): string {
  return {
    gentle: '☕ 休息结束，慢慢回到状态吧',
    motivator: '休息完毕，第二回合，开干！',
    sarcastic: '歇够了？回来干活 🙃',
    cool: '继续。',
  }[p];
}
