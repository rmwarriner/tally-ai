import styles from "./AIMessage.module.css";

interface AIMessageProps {
  text: string;
}

export function AIMessage({ text }: AIMessageProps) {
  return (
    <div className={styles.row}>
      <span className={styles.avatar} aria-label="AI avatar">
        AI
      </span>
      <div className={styles.bubble}>{text}</div>
    </div>
  );
}
