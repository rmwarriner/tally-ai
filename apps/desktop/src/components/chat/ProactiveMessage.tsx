import { AIAvatar } from "./AIAvatar";
import styles from "./ProactiveMessage.module.css";

interface ProactiveMessageProps {
  id: string;
  text: string;
  ts: number;
  advisory_code?: string;
}

export function ProactiveMessage({ text, advisory_code }: ProactiveMessageProps) {
  return (
    <div className={styles.row}>
      <AIAvatar variant="proactive" />
      <div className={styles.bubble} role="note" aria-label="Proactive advisory">
        <div>{text}</div>
        {advisory_code ? <span className={styles.codePill}>{advisory_code}</span> : null}
      </div>
    </div>
  );
}

export type { ProactiveMessageProps };
