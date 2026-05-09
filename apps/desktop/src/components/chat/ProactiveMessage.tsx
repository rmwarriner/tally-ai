import type { RecoveryAction } from "@tally/core-types";

import { AIAvatar } from "./AIAvatar";
import styles from "./ProactiveMessage.module.css";

interface ProactiveMessageProps {
  id: string;
  text: string;
  ts: number;
  advisory_code?: string;
  recovery?: RecoveryAction[];
  category?: "alert" | "insight" | "briefing";
}

export function ProactiveMessage({
  text,
  advisory_code,
  recovery,
  category = "insight",
}: ProactiveMessageProps) {
  const bubbleClass =
    category === "alert"
      ? `${styles.bubble} ${styles.alert}`
      : category === "briefing"
        ? `${styles.bubble} ${styles.briefing}`
        : styles.bubble;
  const ariaLabel =
    category === "briefing"
      ? "Morning briefing"
      : category === "alert"
        ? "Proactive alert"
        : "Proactive advisory";
  return (
    <div className={styles.row}>
      <AIAvatar variant="proactive" />
      <div className={bubbleClass} role="note" aria-label={ariaLabel} data-category={category}>
        <div>{text}</div>
        {advisory_code ? <span className={styles.codePill}>{advisory_code}</span> : null}
        {recovery && recovery.length > 0 ? (
          <ul className={styles.recoveryList} aria-label="Recovery actions">
            {recovery.map((action) => (
              <li
                key={action.kind}
                className={action.is_primary ? styles.recoveryPrimary : styles.recoveryItem}
              >
                {action.label}
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </div>
  );
}

export type { ProactiveMessageProps };
