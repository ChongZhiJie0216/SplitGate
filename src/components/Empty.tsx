import Icon, { IconName } from "./Icon";

/** 没有内容时的说明：一个淡色大图标 + 一句话。 */
export default function Empty({ icon, text }: { icon: IconName; text: string }) {
  return (
    <div className="empty">
      <Icon name={icon} size={36} />
      <p>{text}</p>
    </div>
  );
}
