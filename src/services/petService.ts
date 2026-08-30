import { invoke } from '@tauri-apps/api/core';

/** 桌宠相关事件（Rust → pet 窗口）。 */
export const PET_TASKS_CHANGED_EVENT = 'pet://tasks-changed';
export const PET_TASK_COMPLETED_EVENT = 'pet://task-completed';
export const PET_REMINDER_EVENT = 'pet://reminder';
export const PET_PHOTO_CHANGED_EVENT = 'pet://photo-changed';
export const PET_PERSONALITY_CHANGED_EVENT = 'pet://personality-changed';

export interface PetReminderPayload {
  taskId: number;
  title: string;
  /** DUE_SOON：即将到期；OVERDUE：已逾期 */
  kind: 'DUE_SOON' | 'OVERDUE';
  /** DUE_SOON 时的实际提前量（分钟），气泡文案用它而不是写死 10 */
  leadMinutes: number;
}

/** 桌宠视觉风格（Rust 端 PetStyle）。 */
export type PetStyleId = 'original' | 'chibi' | 'anime' | 'pixel' | 'sketch';

/** 桌宠人格（Rust 端 PetPersonality）。 */
export type PetPersonalityId = 'gentle' | 'motivator' | 'sarcastic' | 'cool';

/** 照片桌宠配置（Rust 端 PetPhotoConfig 的镜像）。 */
export interface PetPhotoConfig {
  /** 抠图+风格处理后的桌宠形象路径，未导入过为 null */
  path: string | null;
  /** 是否启用照片桌宠（false = 默认小猫） */
  enabled: boolean;
  /** 成品形象的 petphoto:// 地址 */
  url: string | null;
  /** 用户上传的原始照片路径 */
  sourcePath: string | null;
  /** 原始照片的 petphoto:// 地址（对照预览用） */
  sourceUrl: string | null;
  style: PetStyleId;
  tolerance: number;
  /** 是否使用 AI 人像分割（false = 几何快速模式） */
  useAi: boolean;
  /** 多帧动画的所有帧地址（第 0 帧即主形象；单帧 = 静态 + 呼吸/眨眼微动画） */
  frames: string[];
  /** 多帧轮播间隔（毫秒） */
  frameMs: number;
  /** 显示边长（px，64~192） */
  petSize: number;
  /** 不透明度（%，20~100） */
  petOpacity: number;
}

export const PET_STYLES: { id: PetStyleId; icon: string; label: string; desc: string }[] = [
  { id: 'original', icon: '🖼️', label: '原图', desc: '只抠掉背景，保留照片原样' },
  { id: 'chibi', icon: '🧸', label: 'Q版贴纸', desc: '中心放大 + 白边贴纸感' },
  { id: 'anime', icon: '🌸', label: '二次元', desc: '赛璐璐平涂色块，动画海报感' },
  { id: 'pixel', icon: '🕹️', label: '像素风', desc: '复古游戏像素点阵' },
  { id: 'sketch', icon: '✏️', label: '手绘风', desc: '铅笔线稿 + 纸面质感' },
];

export const PET_PERSONALITIES: {
  id: PetPersonalityId;
  icon: string;
  label: string;
  desc: string;
}[] = [
  { id: 'gentle', icon: '🌸', label: '温柔型', desc: '细声细气，永远在鼓励你' },
  { id: 'motivator', icon: '⚡', label: '激励型', desc: '热血教练，陪你冲刺每一轮' },
  { id: 'sarcastic', icon: '🌶️', label: '毒舌型', desc: '嘴上嫌弃你，心里盼你好' },
  { id: 'cool', icon: '🌙', label: '高冷型', desc: '话不多，但句句在点' },
];

/**
 * 桌宠数据访问层：所有 invoke / 事件监听都经过这里。
 */
export const petService = {
  setVisible(visible: boolean): Promise<void> {
    return invoke('set_pet_visible', { visible });
  },
  isEnabled(): Promise<boolean> {
    return invoke<boolean>('is_pet_enabled_command');
  },
  getPhoto(): Promise<PetPhotoConfig> {
    return invoke<PetPhotoConfig>('get_pet_photo');
  },
  /** 弹系统文件对话框选照片（自动抠图）；用户取消返回 null。 */
  pickPhoto(): Promise<PetPhotoConfig | null> {
    return invoke<PetPhotoConfig | null>('pick_pet_photo');
  },
  setPhotoEnabled(enabled: boolean): Promise<PetPhotoConfig> {
    return invoke<PetPhotoConfig>('set_pet_photo_enabled', { enabled });
  },
  /** 按新参数重新抠图/换风格/切分割模式；传 null 表示沿用当前值。 */
  reprocess(
    tolerance: number | null,
    style: PetStyleId | null,
    useAi: boolean | null,
  ): Promise<PetPhotoConfig> {
    return invoke<PetPhotoConfig>('reprocess_pet_photo', { tolerance, style, useAi });
  },
  getPersonality(): Promise<PetPersonalityId> {
    return invoke<PetPersonalityId>('get_pet_personality');
  },
  setPersonality(personality: PetPersonalityId): Promise<void> {
    return invoke('set_pet_personality', { personality });
  },
  /** 照片桌宠形象在窗口里的显示区（像素级鼠标穿透命中判定用）。 */
  reportHitRegion(x: number, y: number, width: number, height: number): Promise<void> {
    return invoke('set_pet_hit_region', { x, y, width, height });
  },
  /** 鼠标按下/抬起：拖动期间冻结穿透状态。 */
  setHitPressed(pressed: boolean): Promise<void> {
    return invoke('set_pet_hit_pressed', { pressed });
  },
  /** 追加一帧动画照片（走当前风格/容差处理）；取消返回 null。 */
  addFrame(): Promise<PetPhotoConfig | null> {
    return invoke<PetPhotoConfig | null>('add_pet_frame');
  },
  /** 删除附加帧（index 对应 frames 数组下标，0 = 主形象不可删）。 */
  removeFrame(index: number): Promise<PetPhotoConfig> {
    return invoke<PetPhotoConfig>('remove_pet_frame', { index });
  },
  /** 多帧轮播间隔（毫秒，100~2000）。 */
  setFrameSpeed(ms: number): Promise<void> {
    return invoke('set_pet_frame_speed', { ms });
  },
  /** 显示参数：边长 64~192px、不透明度 20~100%。 */
  setDisplay(size: number, opacity: number): Promise<void> {
    return invoke('set_pet_display', { size, opacity });
  },
};
