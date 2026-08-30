import { useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { useTaskStore } from '../../stores/taskStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { useFocusStore } from '../../stores/focusStore';
import {
  PET_PERSONALITY_CHANGED_EVENT,
  PET_PHOTO_CHANGED_EVENT,
  PET_REMINDER_EVENT,
  PET_TASK_COMPLETED_EVENT,
  PET_TASKS_CHANGED_EVENT,
  petService,
  type PetPersonalityId,
  type PetReminderPayload,
} from '../../services/petService';
import { FOCUS_CHANGED_EVENT } from '../../services/focusService';
import type { FocusState } from '../../services/focusService';
import { AI_REPLY_EVENT } from '../../services/aiService';
import {
  deriveMood,
  focusBreakEndMessage,
  focusDoneMessage,
  focusStartMessage,
  greetingMessage,
  reminderMessage,
  taskDoneMessage,
  type PetMood,
} from '../../utils/petMood';
import { PetSprite } from './PetSprite';

const TEMP_MOOD_MS = 4000;
const BUBBLE_MS = 6000;
/** 区分「点击」和「拖动」：移动超过该距离立刻进入系统拖动，没超过就抬起视为点击。 */
const DRAG_THRESHOLD_PX = 4;

/**
 * 桌宠页面：透明窗口里一只默认小猫。
 * - 与 Todo 联动：任务完成 → 开心动画；有逾期 → 低落；到期提醒 → 弹气泡
 * - 短按弹今日概览气泡；按住拖动换位置
 */
export function PetPage() {
  const { tasks, load } = useTaskStore();
  const { petPhoto, refreshPetPhoto } = useSettingsStore();
  const { phase: focusPhase, workMinutes, breakMinutes, apply: applyFocus } = useFocusStore();
  const [tempMood, setTempMood] = useState<PetMood | null>(null);
  const [bubble, setBubble] = useState<string | null>(null);
  // 桌宠人格：气泡文案的语气由它决定（桌宠中心里改）
  const [personality, setPersonality] = useState<PetPersonalityId>('gentle');
  // 形象在窗口里的显示区：气泡自动贴到头顶（默认小猫没有该数据时回退窗口顶部）
  const [spriteRect, setSpriteRect] = useState<{
    left: number;
    top: number;
    width: number;
    height: number;
  } | null>(null);
  // 多帧轮播：当前帧下标（单帧时恒 0，静态图走呼吸/眨眼微动画）
  const [frameIdx, setFrameIdx] = useState(0);

  const moodTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const bubbleTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // 事件回调里要读最新人格，用 ref 避免重新注册监听
  const personalityRef = useRef<PetPersonalityId>('gentle');
  personalityRef.current = personality;

  const baseMood = useMemo(() => deriveMood(tasks), [tasks]);
  // 专注中桌宠保持「警觉」神态，陪伴主人工作
  const focusMood: PetMood | null = focusPhase === 'FOCUS' ? 'alert' : null;
  const mood = tempMood ?? focusMood ?? baseMood;

  const showTempMood = (m: PetMood, ms = TEMP_MOOD_MS) => {
    setTempMood(m);
    if (moodTimer.current) clearTimeout(moodTimer.current);
    moodTimer.current = setTimeout(() => setTempMood(null), ms);
  };

  const showBubble = (text: string, ms = BUBBLE_MS) => {
    setBubble(text);
    if (bubbleTimer.current) clearTimeout(bubbleTimer.current);
    bubbleTimer.current = setTimeout(() => setBubble(null), ms);
  };

  useEffect(() => {
    void load();
    void refreshPetPhoto();
    void petService
      .getPersonality()
      .then(setPersonality)
      .catch(() => undefined);
    // 热重启竞态兜底：数据库尚未就绪时首拉可能拿不到照片，稍后再补一次
    const photoRetry = setTimeout(() => void refreshPetPhoto(), 1500);

    const unlistenTasks = listen(PET_TASKS_CHANGED_EVENT, () => {
      void load();
    });
    // 桌宠中心导入/切换照片桌宠后实时换形象
    const unlistenPhoto = listen(PET_PHOTO_CHANGED_EVENT, () => {
      void refreshPetPhoto();
    });
    // 桌宠中心改人格后同步气泡语气
    const unlistenPersonality = listen<string>(PET_PERSONALITY_CHANGED_EVENT, (e) => {
      setPersonality(e.payload as PetPersonalityId);
    });
    // 勾选完成 → 开心 4 秒
    const unlistenDone = listen<string>(PET_TASK_COMPLETED_EVENT, (e) => {
      showTempMood('happy');
      showBubble(taskDoneMessage(e.payload, personalityRef.current));
    });
    // 到期提醒 → 警觉 + 气泡
    const unlistenReminder = listen<PetReminderPayload>(PET_REMINDER_EVENT, (e) => {
      showTempMood('alert', BUBBLE_MS);
      showBubble(reminderMessage(e.payload, personalityRef.current));
    });
    // AI 回复 → 桌宠同步弹气泡（后端已截断，这里限保险长度）
    const unlistenAi = listen<string>(AI_REPLY_EVENT, (e) => {
      const text = e.payload.length > 80 ? `${e.payload.slice(0, 80)}…` : e.payload;
      showTempMood('happy', BUBBLE_MS);
      showBubble(text);
    });
    // 专注阶段切换 → 桌宠播报（开始/完成/休息结束）
    const unlistenFocus = listen<FocusState>(FOCUS_CHANGED_EVENT, (e) => {
      applyFocus(e.payload);
    });

    return () => {
      unlistenTasks.then((f) => f());
      unlistenPhoto.then((f) => f());
      unlistenPersonality.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenReminder.then((f) => f());
      unlistenAi.then((f) => f());
      unlistenFocus.then((f) => f());
      clearTimeout(photoRetry);
      if (moodTimer.current) clearTimeout(moodTimer.current);
      if (bubbleTimer.current) clearTimeout(bubbleTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 专注阶段变化 → 桌宠播报气泡（对比上一个阶段决定说什么）
  const prevPhase = useRef(focusPhase);
  useEffect(() => {
    const prev = prevPhase.current;
    prevPhase.current = focusPhase;
    if (prev === focusPhase) return;
    if (focusPhase === 'FOCUS') {
      showBubble(focusStartMessage(workMinutes, personalityRef.current));
    } else if (focusPhase === 'BREAK') {
      showTempMood('happy', BUBBLE_MS);
      showBubble(focusDoneMessage(breakMinutes, personalityRef.current));
    } else if (prev === 'BREAK') {
      showBubble(focusBreakEndMessage(personalityRef.current));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusPhase]);

  // 照片桌宠参数：尺寸/透明度/帧
  const petSize = petPhoto?.petSize ?? 96;
  const petOpacity = (petPhoto?.petOpacity ?? 100) / 100;
  const frames = petPhoto?.enabled ? (petPhoto?.frames ?? []) : [];
  const multiFrame = frames.length > 1;

  // 多帧轮播：按后端配置的间隔切换
  useEffect(() => {
    if (!multiFrame) {
      setFrameIdx(0);
      return;
    }
    const ms = petPhoto?.frameMs ?? 300;
    const timer = setInterval(() => setFrameIdx((i) => (i + 1) % frames.length), ms);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [multiFrame, frames.length, petPhoto?.frameMs]);

  // 窗口随形象尺寸自适应（底边位置保持，向上生长）
  const prevSize = useRef(petSize);
  useEffect(() => {
    if (prevSize.current === petSize) return;
    prevSize.current = petSize;
    const win = getCurrentWindow();
    void (async () => {
      try {
        const [pos, cur, factor] = await Promise.all([
          win.outerPosition(),
          win.outerSize(),
          win.scaleFactor(),
        ]);
        const bottom = pos.y + cur.height;
        await win.setSize(new LogicalSize(petSize + 64, petSize + 104));
        await win.setPosition(
          new PhysicalPosition(pos.x, bottom - Math.round((petSize + 104) * factor)),
        );
      } catch {
        // 调整失败保持原窗口大小，不影响显示
      }
    })();
  }, [petSize]);

  // 点击 vs 拖动：按下后移动超过阈值 → 立即系统级拖动（跟手，无延迟）；
  // 位移没超过阈值就抬起 → 点击（弹气泡）。监听挂在 document 上，
  // 即使鼠标快速甩出桌宠区域也能继续收到移动事件。
  const pressOrigin = useRef<{ x: number; y: number } | null>(null);
  const dragging = useRef(false);

  // 照片桌宠的显示区上报给后端做像素级命中（人物外穿透到桌面），
  // 同时气泡用它贴着头顶定位
  const imgRef = useRef<HTMLImageElement | null>(null);
  const reportHit = () => {
    const el = imgRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setSpriteRect({ left: r.left, top: r.top, width: r.width, height: r.height });
    void petService.reportHitRegion(r.left, r.top, r.width, r.height).catch(() => undefined);
  };
  useEffect(() => {
    window.addEventListener('resize', reportHit);
    return () => window.removeEventListener('resize', reportHit);
  }, []);

  const onPress = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    void petService.setHitPressed(true).catch(() => undefined);
    pressOrigin.current = { x: e.screenX, y: e.screenY };
    dragging.current = false;
  };

  const onRelease = () => {
    if (pressOrigin.current && !dragging.current) {
      showBubble(greetingMessage(tasks, personalityRef.current));
    }
    void petService.setHitPressed(false).catch(() => undefined);
    pressOrigin.current = null;
    dragging.current = false;
  };

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const origin = pressOrigin.current;
      if (!origin || dragging.current) return;
      const dx = e.screenX - origin.x;
      const dy = e.screenY - origin.y;
      if (dx * dx + dy * dy > DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) {
        dragging.current = true;
        void getCurrentWindow().startDragging();
      }
    };
    document.addEventListener('mousemove', onMove);
    return () => document.removeEventListener('mousemove', onMove);
  }, []);

  // 当前展示的帧：多帧轮播；单帧即主形象（静态 + 呼吸/眨眼）
  const frameUrl = multiFrame ? frames[frameIdx % frames.length] : petPhoto?.enabled ? petPhoto.url : null;

  return (
    <div className="fixed inset-0 select-none overflow-hidden">
      {bubble && (
        <div
          role="status"
          style={
            spriteRect
              ? {
                  // 气泡底边贴着抠出人物的头顶，文字多时向上生长；
                  // 左右夹在窗口内不被裁切
                  left: Math.min(
                    Math.max(spriteRect.left + spriteRect.width / 2, 77),
                    window.innerWidth - 77,
                  ),
                  bottom: window.innerHeight - spriteRect.top + 6,
                }
              : { left: '50%', top: 4 }
          }
          className="bubble-pop absolute w-[150px] -translate-x-1/2 rounded-xl bg-white/95 px-2.5 py-2 text-center text-xs leading-relaxed text-zinc-700 shadow-lg ring-1 ring-black/10 dark:bg-zinc-800/95 dark:text-zinc-100 dark:ring-white/10"
        >
          {bubble}
        </div>
      )}
      <div
        onMouseDown={onPress}
        onMouseUp={onRelease}
        className="absolute bottom-1 left-1/2 -translate-x-1/2 cursor-grab active:cursor-grabbing"
        title="FloatDo 桌宠"
      >
        {frameUrl ? (
          // 抠好图的人物形象直接裸显示：外层呼吸起伏，
          // 单帧时内层叠加眨眼轻压，多帧时由帧轮播承担动画
          <div style={{ opacity: petOpacity }}>
            <div className={`origin-bottom ${multiFrame ? '' : 'pet-breathe'}`}>
              <img
                ref={imgRef}
                key={frameUrl}
                src={frameUrl}
                alt="照片桌宠"
                draggable={false}
                onLoad={reportHit}
                style={{ width: petSize, height: petSize }}
                className={`origin-bottom object-contain drop-shadow-[0_2px_6px_rgba(0,0,0,0.35)] ${
                  multiFrame ? '' : 'pet-photo-blink'
                }`}
              />
            </div>
          </div>
        ) : (
          <PetSprite mood={mood} />
        )}
      </div>
    </div>
  );
}
