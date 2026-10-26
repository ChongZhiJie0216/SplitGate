import { useApp } from "../store/app";
import { useProfiles } from "../store/profiles";
import { useStatus } from "../store/status";
import { T } from "../lib/text";

/** 顶部的连接按钮：文字和颜色只跟着后端状态走。 */
export default function ConnectButton({ big = false }: { big?: boolean }) {
  const { status, connect, disconnect } = useStatus();
  const info = useApp((s) => s.info);
  const hasProfile = !!useProfiles((s) => s.view?.active);
  const cls = `connect-btn ${big ? "big " : ""}${status.state}`;

  switch (status.state) {
    case "connected":
      return (
        <button className={cls} onClick={disconnect}>
          {T.action.disconnect}
        </button>
      );
    case "connecting":
      return (
        <button className={cls} onClick={disconnect} title={T.action.cancel}>
          {T.steps[status.step]}… <span className="btn-sub">{T.action.cancel}</span>
        </button>
      );
    case "disconnecting":
      return (
        <button className={cls} disabled>
          {T.status.disconnecting}…
        </button>
      );
    default: {
      const retrying = status.state === "error" && status.retrying;
      return (
        <button
          className={retrying ? `connect-btn ${big ? "big " : ""}connecting` : cls}
          onClick={connect}
          disabled={!info?.isAdmin || !hasProfile}
        >
          {retrying ? T.action.retryNow : status.state === "error" ? T.action.retry : T.action.connect}
        </button>
      );
    }
  }
}
