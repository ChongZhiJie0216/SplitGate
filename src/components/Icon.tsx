// 线条图标（v0.22.0）：自己画的 24×24 SVG，不加依赖；颜色跟着文字（currentColor），亮 / 暗主题都适用。

const PATHS = {
  power: "M12 3v8 M6.3 6.3a8 8 0 1 0 11.4 0",
  rules: "M4 5h16 M7 12h10 M10 19h4",
  connections: "M4 7h11 M4 12h16 M4 17h8 M18 5l2 2-2 2 M16 15l2 2-2 2",
  canvas: "M6 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z M18 3.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z M18 15.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z M8.3 10.8l7.4-3.6 M8.3 13.2l7.4 3.6",
  blocked: "M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6l7-3z M12 8v4 M12 15.5v.5",
  logs: "M7 3h7l5 5v13H7z M14 3v5h5 M10 13h6 M10 17h6",
  profiles: "M4 5h16v6H4z M4 13h16v6H4z M8 8h.01 M8 16h.01",
  settings:
    "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z M19.4 13.5l1.6 1.2-2 3.5-1.9-.7a7 7 0 0 1-1.7 1l-.3 2h-4l-.3-2a7 7 0 0 1-1.7-1l-1.9.7-2-3.5 1.6-1.2a7 7 0 0 1 0-3L3 9.3l2-3.5 1.9.7a7 7 0 0 1 1.7-1l.3-2h4l.3 2a7 7 0 0 1 1.7 1l1.9-.7 2 3.5-1.6 1.2a7 7 0 0 1 0 3z",
  about: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M12 11v5 M12 8v.01",
  plus: "M12 5v14 M5 12h14",
  trash: "M4 7h16 M9 7V4h6v3 M6 7l1 13h10l1-13 M10 11v6 M14 11v6",
  refresh: "M20 11a8 8 0 0 0-14.3-4.7L4 8 M4 4v4h4 M4 13a8 8 0 0 0 14.3 4.7L20 16 M20 20v-4h-4",
  download: "M12 4v11 M7 10l5 5 5-5 M5 20h14",
  upload: "M12 20V9 M7 14l5-5 5 5 M5 4h14",
  check: "M5 12.5l4.5 4.5L19 7",
  x: "M6 6l12 12 M18 6L6 18",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14z M20 20l-4-4",
  pause: "M9 5v14 M15 5v14",
  play: "M8 5l11 7-11 7z",
  folder: "M3 6h6l2 2h10v11H3z",
  shield: "M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6l7-3z M9 12l2 2 4-4",
  globe: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M3 12h18 M12 3c2.5 2.7 3.5 5.7 3.5 9s-1 6.3-3.5 9c-2.5-2.7-3.5-5.7-3.5-9s1-6.3 3.5-9z",
  home: "M4 11l8-7 8 7 M6 9.5V20h12V9.5 M10 20v-6h4v6",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M12 7v5l3 2",
  gauge: "M4 18a8 8 0 1 1 16 0 M12 18l4-6",
  arrows: "M7 4v16 M3 8l4-4 4 4 M17 20V4 M13 16l4 4 4-4",
  key: "M8 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z M11.5 12H21 M18 12v3 M15 12v2",
  alert: "M12 4l9 16H3z M12 10v4 M12 17v.01",
  history: "M3.5 12a8.5 8.5 0 1 0 2.5-6 M3 4v4.5h4.5 M12 8v4l3 2",
  scale: "M12 4v16 M8 20h8 M5 7h14 M7 7l-3 6a3 3 0 0 0 6 0z M17 7l-3 6a3 3 0 0 0 6 0z",
} as const;

export type IconName = keyof typeof PATHS;

export default function Icon({ name, size = 18, className }: { name: IconName; size?: number; className?: string }) {
  return (
    <svg
      className={className ? `icon ${className}` : "icon"}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
