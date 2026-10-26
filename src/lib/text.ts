// 界面文案集中放在这里，以后加英文时只改这个文件。
export const T = {
  appName: "SplitGate",
  nav: {
    connect: "连接",
    rules: "规则",
    logs: "日志",
    profiles: "Profile",
    settings: "设置",
    about: "关于",
  },
  status: {
    disconnected: "已断开",
    connecting: "连接中",
    connected: "已连接",
    disconnecting: "断开中",
    error: "错误",
  },
  notAdmin: "没有以管理员身份运行：可以浏览和编辑设置，但不能连接。",
  comingSoon: "这一页还在开发（第 1 阶段）。",
  about: {
    version: "版本",
    components: "组件版本",
    licenses: "许可证与第三方声明见安装目录的 third_party 文件夹。",
  },
  theme: { auto: "自动", light: "亮", dark: "暗", label: "主题" },
} as const;
