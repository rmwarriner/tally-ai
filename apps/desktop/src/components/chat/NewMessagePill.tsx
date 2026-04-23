import styles from "./NewMessagePill.module.css";

interface NewMessagePillProps {
  onClick: () => void;
}

export function NewMessagePill({ onClick }: NewMessagePillProps) {
  return (
    <button type="button" className={styles.pill} onClick={onClick} aria-label="New message">
      ↓ New message
    </button>
  );
}
