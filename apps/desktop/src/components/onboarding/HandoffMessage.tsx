import styles from "./HandoffMessage.module.css";
import { InfoCircle } from "../ui/InfoCircle";

export interface HandoffMessageProps {
  householdName: string;
  accountCount: number;
  envelopeCount: number;
  starterPrompts: string[];
  onPromptClick: (prompt: string) => void;
}

export function HandoffMessage({
  householdName,
  accountCount,
  envelopeCount,
  starterPrompts,
  onPromptClick,
}: HandoffMessageProps) {
  return (
    <div className={styles.wrapper}>
      <p className={styles.headline}>
        <strong>{householdName}</strong> is ready to go.
      </p>
      <p className={styles.summary}>
        {accountCount} {accountCount === 1 ? "account" : "accounts"} ·{" "}
        {envelopeCount} {envelopeCount === 1 ? "envelope" : "envelopes"} · encrypted
      </p>
      <p className={styles.label}>Try saying:</p>
      <ul className={styles.prompts}>
        {starterPrompts.map((prompt) => (
          <li key={prompt}>
            <button
              type="button"
              className={styles.promptBtn}
              onClick={() => onPromptClick(prompt)}
            >
              <span className={styles.promptText}>{prompt}</span>
              <InfoCircle aria-label="tap to use" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
