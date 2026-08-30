import type { PetMood } from '../../utils/petMood';

/**
 * 默认桌宠：一只圆滚滚的小橘猫（纯 SVG，无外部资源）。
 * 表情与动画随 mood 切换：idle 平静、happy 开心、sad 低落、alert 提醒。
 */
export function PetSprite({ mood }: { mood: PetMood }) {
  return (
    <div className={`pet-${mood}`} style={{ width: 120, height: 120 }}>
      <svg viewBox="0 0 120 120" width="120" height="120" aria-hidden>
        {/* 耳朵 */}
        <polygon points="30,38 38,10 52,30" fill="#f59e0b" />
        <polygon points="90,38 82,10 68,30" fill="#f59e0b" />
        <polygon points="34,32 39,18 47,29" fill="#fde68a" />
        <polygon points="86,32 81,18 73,29" fill="#fde68a" />

        {/* 身体 */}
        <ellipse cx="60" cy="70" rx="38" ry="34" fill="#fbbf24" />
        <ellipse cx="60" cy="80" rx="26" ry="22" fill="#fde68a" />

        {/* 尾巴 */}
        <path
          d="M 96 78 Q 112 74 108 58"
          stroke="#f59e0b"
          strokeWidth="8"
          strokeLinecap="round"
          fill="none"
        />

        {mood === 'happy' ? (
          <>
            {/* 开心：弯弯的 ^^ 眼 + 腮红 + 大笑 */}
            <g className="pet-eyes">
              <path d="M 38 62 Q 44 54 50 62" stroke="#78350f" strokeWidth="3.5" strokeLinecap="round" fill="none" />
              <path d="M 70 62 Q 76 54 82 62" stroke="#78350f" strokeWidth="3.5" strokeLinecap="round" fill="none" />
            </g>
            <ellipse cx="34" cy="72" rx="6" ry="3.5" fill="#fb7185" opacity="0.7" />
            <ellipse cx="86" cy="72" rx="6" ry="3.5" fill="#fb7185" opacity="0.7" />
            <path d="M 50 76 Q 60 88 70 76" stroke="#78350f" strokeWidth="3.5" strokeLinecap="round" fill="none" />
          </>
        ) : mood === 'sad' ? (
          <>
            {/* 低落：耷拉眼 + 抿嘴 */}
            <g className="pet-eyes">
              <path d="M 38 60 Q 44 66 50 60" stroke="#78350f" strokeWidth="3.5" strokeLinecap="round" fill="none" />
              <path d="M 70 60 Q 76 66 82 60" stroke="#78350f" strokeWidth="3.5" strokeLinecap="round" fill="none" />
            </g>
            <path d="M 52 80 Q 60 74 68 80" stroke="#78350f" strokeWidth="3" strokeLinecap="round" fill="none" />
            <ellipse cx="30" cy="88" rx="2.5" ry="4" fill="#60a5fa" opacity="0.8" />
          </>
        ) : mood === 'alert' ? (
          <>
            {/* 提醒：瞪大眼睛 + o 嘴 */}
            <g className="pet-eyes">
              <circle cx="44" cy="62" r="7" fill="#78350f" />
              <circle cx="46" cy="59.5" r="2.2" fill="#fff" />
              <circle cx="76" cy="62" r="7" fill="#78350f" />
              <circle cx="78" cy="59.5" r="2.2" fill="#fff" />
            </g>
            <ellipse cx="60" cy="80" rx="4.5" ry="6" fill="#78350f" />
          </>
        ) : (
          <>
            {/* 平静：圆点眼 + 小 w 嘴，自动眨眼 */}
            <g className="pet-eyes">
              <circle cx="44" cy="62" r="4.5" fill="#78350f" />
              <circle cx="76" cy="62" r="4.5" fill="#78350f" />
            </g>
            <path d="M 52 76 Q 56 80 60 76 Q 64 80 68 76" stroke="#78350f" strokeWidth="3" strokeLinecap="round" fill="none" />
          </>
        )}

        {/* 鼻子 */}
        <polygon points="57,70 63,70 60,74" fill="#78350f" />
      </svg>
    </div>
  );
}
