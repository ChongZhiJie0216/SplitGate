// 程序标志（导航左上角），和 exe / 托盘图标是同一个图形（src-tauri/icons，v0.22.0）。

export default function Logo({ size = 26 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <defs>
        <linearGradient id="sg-logo" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#17a596" />
          <stop offset="1" stopColor="#0b6b62" />
        </linearGradient>
      </defs>
      <rect x="24" y="24" width="464" height="464" rx="112" fill="url(#sg-logo)" />
      <path d="M96 256 H232" stroke="#fff" strokeWidth="44" strokeLinecap="round" fill="none" />
      <path d="M232 256 C 300 256, 316 160, 400 150" stroke="#fff" strokeWidth="44" strokeLinecap="round" fill="none" />
      <path d="M232 256 C 300 256, 316 352, 400 362" stroke="#bfeee8" strokeOpacity="0.75" strokeWidth="44" strokeLinecap="round" fill="none" />
      <circle cx="232" cy="256" r="30" fill="#fff" />
    </svg>
  );
}
