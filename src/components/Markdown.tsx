import { Fragment, ReactNode } from "react";

/** `代码` 和 **粗体**；其他原样显示（内容来自 CHANGELOG.md，不当 HTML 解析）。 */
function inline(text: string): ReactNode[] {
  return text.split(/(`[^`]+`|\*\*[^*]+\*\*)/).map((part, i) => {
    if (part.startsWith("`") && part.endsWith("`") && part.length > 1) return <code key={i}>{part.slice(1, -1)}</code>;
    if (part.startsWith("**") && part.endsWith("**") && part.length > 3) return <b key={i}>{part.slice(2, -2)}</b>;
    return <Fragment key={i}>{part}</Fragment>;
  });
}

/** 只认 CHANGELOG 用到的写法：### 标题、- 列表（缩进两格是下一层）、普通段落。 */
export default function Markdown({ text }: { text: string }) {
  const out: ReactNode[] = [];
  text.split("\n").forEach((line, i) => {
    if (!line.trim()) return;
    const h = line.match(/^#{3,6}\s+(.*)$/);
    const li = line.match(/^(\s*)[-*]\s+(.*)$/);
    if (h) out.push(<h4 key={i}>{inline(h[1])}</h4>);
    else if (li) out.push(<div key={i} className={li[1].length >= 2 ? "md-li sub" : "md-li"}>{inline(li[2])}</div>);
    else out.push(<p key={i}>{inline(line.trim())}</p>);
  });
  return <div className="md">{out}</div>;
}
