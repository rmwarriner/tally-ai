import { AIAvatar } from "./AIAvatar";
import styles from "./AIMessage.module.css";

interface AIMessageProps {
  text: string;
}

export function AIMessage({ text }: AIMessageProps) {
  return (
    <div className={styles.row}>
      <AIAvatar variant="standard" />
      <div className={styles.bubble}>{text}</div>
    </div>
  );
}
