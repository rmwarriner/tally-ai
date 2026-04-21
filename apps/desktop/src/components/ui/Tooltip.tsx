import styles from "./Tooltip.module.css";

interface TooltipProps {
  id: string;
  text: string;
  visible: boolean;
}

export function Tooltip({ id, text, visible }: TooltipProps) {
  return (
    <span
      id={id}
      role="tooltip"
      className={`${styles.tooltip} ${visible ? styles.visible : ""}`.trim()}
    >
      {text}
    </span>
  );
}
