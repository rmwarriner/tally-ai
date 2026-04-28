import type { RecoveryAction } from "@tally/core-types";

import { AIAvatar } from "./AIAvatar";
import styles from "./ProactiveMessage.module.css";

interface ProactiveMessageProps {
  id: string;
  text: string;
  ts: number;
  advisory_code?: string;
  recovery?: RecoveryAction[];
}

export function ProactiveMessage({ text, advisory_code, recovery }: ProactiveMessageProps) {
  return (
    <div className={styles.row}>
      <AIAvatar variant="proactive" />
      <div className={styles.bubble} role="note" aria-label="Proactive advisory">
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
