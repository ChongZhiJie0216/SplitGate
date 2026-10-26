// 规则集列表。后端是唯一来源：每次修改后用后端返回的结果整个替换。
import { create } from "zustand";
import * as ipc from "../lib/ipc";

interface RulesState {
  rulesets: ipc.RuleSetView[] | null;
  error: string | null;
  load: () => Promise<void>;
  /** 执行一次修改；成功返回 null，失败返回中文错误。 */
  run: (op: () => Promise<ipc.RuleSetView[]>) => Promise<string | null>;
}

export const useRules = create<RulesState>((set) => ({
  rulesets: null,
  error: null,
  load: async () => {
    try {
      set({ rulesets: await ipc.listRulesets(), error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  run: async (op) => {
    try {
      set({ rulesets: await op(), error: null });
      return null;
    } catch (e) {
      return String(e);
    }
  },
}));
