import styles from "./UserMessage.module.css";

interface UserMessageProps {
  text: string;
}

export function UserMessage({ text }: UserMessageProps) {
  return (
    <div className={styles.row}>
      <div className={styles.bubble}>{text}</div>
    </div>
  );
}
