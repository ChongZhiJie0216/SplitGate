// 主题：自动（跟随系统）/ 亮 / 暗。选择存在 localStorage，读写失败时退回“自动”。
export type Theme = "auto" | "light" | "dark";

const KEY = "splitgate.theme";

export function loadTheme(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark" || v === "auto") return v;
  } catch {
    // 存储不可用时用默认值
  }
  return "auto";
}

export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // 忽略
  }
}
