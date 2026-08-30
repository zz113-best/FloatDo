import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSettingsStore } from '../../stores/settingsStore';
import {
  petService,
  PET_PHOTO_CHANGED_EVENT,
  PET_PERSONALITIES,
  PET_PERSONALITY_CHANGED_EVENT,
  PET_STYLES,
  type PetPersonalityId,
  type PetStyleId,
} from '../../services/petService';

/**
 * 桌宠中心：桌宠显示开关、照片上传 + 抠图 + 视觉风格、人格系统。
 * 与设置 / 统计 / AI 对话平级的主面板页签。
 */
export function PetCenterPage() {
  const { petPhoto, petEnabled, setPetEnabled, refreshPetPhoto } = useSettingsStore();
  // 抠图容差本地草稿：拖动过程不写库，松手才重新抠图
  const [toleranceDraft, setToleranceDraft] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [personality, setPersonality] = useState<PetPersonalityId | null>(null);

  useEffect(() => {
    void petService
      .getPersonality()
      .then(setPersonality)
      .catch(() => setPersonality('gentle'));
    // 其他窗口（如桌宠窗口）触发的照片变更也要同步
    const unlistenPhoto = listen(PET_PHOTO_CHANGED_EVENT, () => {
      void refreshPetPhoto();
    });
    const unlistenPersonality = listen<string>(PET_PERSONALITY_CHANGED_EVENT, (e) => {
      setPersonality(e.payload as PetPersonalityId);
    });
    return () => {
      unlistenPhoto.then((f) => f());
      unlistenPersonality.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setToleranceDraft(null);
  }, [petPhoto?.tolerance]);

  const pickPhoto = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await petService.pickPhoto();
      if (result) await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
      // 失败时源图已保留，刷新配置好让用户能调容差重试
      await refreshPetPhoto();
    } finally {
      setBusy(false);
    }
  };

  const reprocess = async (
    tolerance: number | null,
    style: PetStyleId | null,
    useAi: boolean | null = null,
  ) => {
    setBusy(true);
    setError(null);
    try {
      await petService.reprocess(tolerance, style, useAi);
      await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const setPhotoEnabled = async (value: boolean) => {
    setError(null);
    try {
      await petService.setPhotoEnabled(value);
      await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
    }
  };

  const choosePersonality = async (id: PetPersonalityId) => {
    const prev = personality;
    setPersonality(id); // 先切 UI，失败回滚
    try {
      await petService.setPersonality(id);
    } catch (e) {
      setPersonality(prev);
      setError(String(e));
    }
  };

  // 显示参数：拖动过程用草稿，松手才落库（大小/透明度）
  const [sizeDraft, setSizeDraft] = useState<number | null>(null);
  const [opacityDraft, setOpacityDraft] = useState<number | null>(null);
  const petSize = petPhoto?.petSize ?? 96;
  const petOpacity = petPhoto?.petOpacity ?? 100;

  const commitDisplay = async (size: number | null, opacity: number | null) => {
    setError(null);
    try {
      await petService.setDisplay(
        size ?? petSize,
        opacity ?? petOpacity,
      );
      await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
    }
  };

  // 多帧动画
  const addFrame = async () => {
    setBusy(true);
    setError(null);
    try {
      const config = await petService.addFrame();
      if (config) await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const removeFrame = async (index: number) => {
    setBusy(true);
    setError(null);
    try {
      await petService.removeFrame(index);
      await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const changeFrameSpeed = async (ms: number) => {
    setError(null);
    try {
      await petService.setFrameSpeed(ms);
      await refreshPetPhoto();
    } catch (e) {
      setError(String(e));
    }
  };

  const tolerance = toleranceDraft ?? petPhoto?.tolerance ?? 30;
  const hasSource = Boolean(petPhoto?.sourceUrl);

  return (
    <div className="p-6">
      <div className="mx-auto max-w-4xl space-y-4">
        <h1 className="text-lg font-semibold">桌宠中心</h1>

        {/* 双栏：左边形象（上传/抠图/风格），右边人格与显示开关 */}
        <div className="grid grid-cols-2 items-start gap-4">
          <div>

        {/* 桌宠形象预览 + 上传 / 抠图 / 风格 */}
        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 text-sm font-medium">我的桌宠形象</h2>
          <div className="flex items-start gap-4">
            <div className="flex h-20 w-20 shrink-0 items-center justify-center overflow-hidden rounded-full bg-zinc-100 ring-2 ring-black/10 dark:bg-zinc-700 dark:ring-white/10">
              {/* 抠图失败时成品还没有，先展示源图让用户知道照片已导入 */}
              {petPhoto?.url ?? petPhoto?.sourceUrl ? (
                <img
                  src={(petPhoto?.url ?? petPhoto?.sourceUrl) as string}
                  alt="桌宠形象预览"
                  className="h-full w-full object-contain"
                />
              ) : (
                <span className="text-3xl" aria-hidden>🐱</span>
              )}
            </div>
            <div className="min-w-0 flex-1 space-y-2">
              <button
                type="button"
                onClick={() => void pickPhoto()}
                disabled={busy}
                className="rounded-md bg-blue-500 px-3 py-1.5 text-sm text-white transition hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {busy ? '处理中…' : hasSource ? '更换照片…' : '上传个人照片…'}
              </button>
              {petPhoto?.path && (
                <label className="flex cursor-pointer items-center gap-2">
                  <input
                    type="checkbox"
                    checked={Boolean(petPhoto?.enabled)}
                    onChange={(e) => void setPhotoEnabled(e.target.checked)}
                    className="h-4 w-4 accent-blue-500"
                  />
                  <span className="text-sm">使用照片桌宠（关闭则显示默认小猫）</span>
                </label>
              )}
            </div>
          </div>

          {hasSource && (
            <>
              <label className="mt-4 flex cursor-pointer items-center gap-2">
                <input
                  type="checkbox"
                  checked={petPhoto?.useAi ?? true}
                  onChange={(e) => void reprocess(null, null, e.target.checked)}
                  disabled={busy}
                  className="h-4 w-4 accent-blue-500"
                />
                <span className="text-sm">AI 人像分割（推荐，复杂背景也能精准抠出人物）</span>
              </label>
              <p className="mt-1 text-xs leading-relaxed text-zinc-400">
                首次使用会自动下载约 4.5MB 的分割模型（之后离线可用）；
                关闭则用快速几何抠图，只适合干净背景。
              </p>

              <div className="mt-4 flex items-center justify-between gap-3">
                <span className="shrink-0 text-sm">抠图容差</span>
                <div className="flex flex-1 items-center gap-2">
                  <input
                    type="range"
                    min={5}
                    max={120}
                    value={tolerance}
                    onChange={(e) => setToleranceDraft(Number(e.target.value))}
                    onMouseUp={() => void reprocess(tolerance, null)}
                    onTouchEnd={() => void reprocess(tolerance, null)}
                    onKeyUp={() => void reprocess(tolerance, null)}
                    className="w-full"
                  />
                  <span className="w-8 text-right text-xs tabular-nums">{tolerance}</span>
                </div>
                <button
                  type="button"
                  onClick={() => void reprocess(tolerance, null)}
                  disabled={busy}
                  className="shrink-0 rounded-md bg-zinc-100 px-3 py-1.5 text-sm text-zinc-700 transition hover:bg-zinc-200 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-zinc-700 dark:text-zinc-200 dark:hover:bg-zinc-600"
                >
                  重新抠图
                </button>
              </div>
              <p className="mt-1 text-xs leading-relaxed text-zinc-400">
                会自动从照片里抠出人物、去掉背景。背景越干净效果越好；
                抠多了就把容差调低，没抠干净就调高。
              </p>

              <h3 className="mb-2 mt-4 text-sm font-medium">视觉风格</h3>
              <div className="grid grid-cols-3 gap-2">
                {PET_STYLES.map((s) => (
                  <button
                    key={s.id}
                    type="button"
                    title={s.desc}
                    onClick={() => void reprocess(null, s.id)}
                    disabled={busy}
                    className={`rounded-lg border p-2 text-center transition disabled:cursor-not-allowed disabled:opacity-50 ${
                      petPhoto?.style === s.id
                        ? 'border-blue-500 bg-blue-50 dark:border-blue-400 dark:bg-blue-500/10'
                        : 'border-black/10 hover:border-blue-300 dark:border-white/10 dark:hover:border-blue-500/50'
                    }`}
                  >
                    <span className="block text-xl" aria-hidden>{s.icon}</span>
                    <span className="mt-1 block text-xs">{s.label}</span>
                  </button>
                ))}
              </div>
              <p className="mt-2 text-xs leading-relaxed text-zinc-400">
                风格是本机实时处理的：{PET_STYLES.find((s) => s.id === petPhoto?.style)?.desc}
              </p>
            </>
          )}
          {!hasSource && (
            <p className="mt-3 text-xs leading-relaxed text-zinc-400">
              上传一张生活照，会自动从中抠出人物（不是简单把整张照片贴成桌宠），
              还能套用不同视觉风格。照片仅保存在本机。
            </p>
          )}
          {error && <p className="mt-2 text-xs text-red-500">{error}</p>}
        </section>

          </div>

          <div className="space-y-4">
            {/* 人格系统 */}
            <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-1 text-sm font-medium">人格</h2>
          <p className="mb-3 text-xs text-zinc-400">
            决定桌宠气泡的说话语气，AI 对话里也是同一个人设。
          </p>
          <div className="grid grid-cols-2 gap-2">
            {PET_PERSONALITIES.map((p) => (
              <button
                key={p.id}
                type="button"
                onClick={() => void choosePersonality(p.id)}
                className={`rounded-lg border p-3 text-left transition ${
                  personality === p.id
                    ? 'border-blue-500 bg-blue-50 dark:border-blue-400 dark:bg-blue-500/10'
                    : 'border-black/10 hover:border-blue-300 dark:border-white/10 dark:hover:border-blue-500/50'
                }`}
              >
                <div className="flex items-center gap-2">
                  <span aria-hidden>{p.icon}</span>
                  <span className="text-sm font-medium">{p.label}</span>
                  {personality === p.id && <span className="ml-auto text-xs text-blue-500">✓</span>}
                </div>
                <p className="mt-1 text-xs leading-relaxed text-zinc-400">{p.desc}</p>
              </button>
            ))}
          </div>
        </section>

        {/* 显示开关 + 显示调节 */}
        <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
          <h2 className="mb-3 text-sm font-medium">显示</h2>
          <label className="flex cursor-pointer items-center justify-between">
            <span className="text-sm">显示桌宠</span>
            <input
              type="checkbox"
              checked={petEnabled}
              onChange={(e) => void setPetEnabled(e.target.checked)}
              className="h-4 w-4 accent-blue-500"
            />
          </label>
          {hasSource && (
            <>
              <div className="mt-4 flex items-center gap-3">
                <span className="w-14 shrink-0 text-sm">大小</span>
                <input
                  type="range"
                  min={64}
                  max={192}
                  step={8}
                  value={sizeDraft ?? petSize}
                  onChange={(e) => setSizeDraft(Number(e.target.value))}
                  onMouseUp={() => void commitDisplay(sizeDraft, null)}
                  onTouchEnd={() => void commitDisplay(sizeDraft, null)}
                  onKeyUp={() => void commitDisplay(sizeDraft, null)}
                  className="w-full"
                />
                <span className="w-10 shrink-0 text-right text-xs tabular-nums text-zinc-400">
                  {sizeDraft ?? petSize}px
                </span>
              </div>
              <div className="mt-2 flex items-center gap-3">
                <span className="w-14 shrink-0 text-sm">透明度</span>
                <input
                  type="range"
                  min={20}
                  max={100}
                  step={5}
                  value={opacityDraft ?? petOpacity}
                  onChange={(e) => setOpacityDraft(Number(e.target.value))}
                  onMouseUp={() => void commitDisplay(null, opacityDraft)}
                  onTouchEnd={() => void commitDisplay(null, opacityDraft)}
                  onKeyUp={() => void commitDisplay(null, opacityDraft)}
                  className="w-full"
                />
                <span className="w-10 shrink-0 text-right text-xs tabular-nums text-zinc-400">
                  {opacityDraft ?? petOpacity}%
                </span>
              </div>
            </>
          )}
          <p className="mt-2 text-xs leading-relaxed text-zinc-400">
            桌宠可以按住拖动、点击弹气泡；位置和大小重启后保持。任务完成、到期提醒、专注计时都会同步播报。
          </p>
        </section>

        {/* 多帧动画 */}
        {hasSource && (
          <section className="rounded-xl bg-white p-4 shadow-sm dark:bg-zinc-800">
            <h2 className="mb-1 text-sm font-medium">动画帧</h2>
            <p className="mb-3 text-xs leading-relaxed text-zinc-400">
              添加多张不同姿势的照片做成逐帧动画；只有一帧时自带呼吸和眨眼微动画。
              换风格或重抠时所有帧一起重新处理。
            </p>
            <div className="flex flex-wrap items-center gap-2">
              {(petPhoto?.frames ?? []).map((url, i) => (
                <div key={`${url}-${i}`} className="relative">
                  <img
                    src={url}
                    alt={`第 ${i} 帧`}
                    className="h-14 w-14 rounded-lg border border-black/10 bg-zinc-100 object-contain dark:border-white/10 dark:bg-zinc-700"
                  />
                  {i === 0 ? (
                    <span className="absolute -bottom-1.5 left-1/2 -translate-x-1/2 rounded bg-zinc-600 px-1 text-[10px] text-white">
                      主
                    </span>
                  ) : (
                    <button
                      type="button"
                      onClick={() => void removeFrame(i)}
                      title="删除这一帧"
                      className="absolute -right-1.5 -top-1.5 flex h-4.5 w-4.5 items-center justify-center rounded-full bg-red-500 text-[10px] leading-none text-white transition hover:bg-red-600"
                    >
                      ×
                    </button>
                  )}
                </div>
              ))}
              <button
                type="button"
                onClick={() => void addFrame()}
                disabled={busy || (petPhoto?.frames.length ?? 0) >= 8}
                title="添加一帧"
                className="flex h-14 w-14 items-center justify-center rounded-lg border border-dashed border-black/20 text-xl text-zinc-400 transition hover:border-blue-400 hover:text-blue-500 disabled:cursor-not-allowed disabled:opacity-40 dark:border-white/20"
              >
                ＋
              </button>
            </div>
            {(petPhoto?.frames.length ?? 0) > 1 && (
              <div className="mt-3 flex items-center gap-2 text-sm">
                <span className="text-zinc-400">轮播速度</span>
                <select
                  value={petPhoto?.frameMs ?? 300}
                  onChange={(e) => void changeFrameSpeed(Number(e.target.value))}
                  className="rounded-md border border-black/10 bg-transparent px-1.5 py-1 text-sm outline-none dark:border-white/10"
                >
                  <option value={500}>慢（0.5 秒/帧）</option>
                  <option value={300}>中（0.3 秒/帧）</option>
                  <option value={150}>快（0.15 秒/帧）</option>
                </select>
              </div>
            )}
          </section>
        )}
          </div>
        </div>
      </div>
    </div>
  );
}
