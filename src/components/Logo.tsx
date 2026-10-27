// 程序标志（导航左上角）：和 exe / 托盘图标是同一个图形，原稿在 src-tauri/icons/source.svg（v0.22.0，维护者选的方案 B：
// 门 + 分流，一条线经过“门”后分成两条，黄色那条是走 VPN 的）。改图标时改原稿，再执行 `npx tauri icon src-tauri/icons/source.svg`。

export default function Logo({ size = 26 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <defs>
        <linearGradient id="sg-logo" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#1f2a2a" />
          <stop offset="1" stopColor="#0f1515" />
        </linearGradient>
      </defs>
      <rect x="24" y="24" width="464" height="464" rx="112" fill="url(#sg-logo)" />
      <path d="M136 400 V236 a120 120 0 0 1 240 0 V400" stroke="#2bb3a4" strokeWidth="40" strokeLinecap="round" fill="none" />
      <path d="M256 412 V300" stroke="#ecebe8" strokeWidth="36" strokeLinecap="round" />
      <path d="M256 300 L196 214" stroke="#ecebe8" strokeWidth="36" strokeLinecap="round" />
      <path d="M256 300 L316 214" stroke="#f2a900" strokeWidth="36" strokeLinecap="round" />
    </svg>
  );
}
