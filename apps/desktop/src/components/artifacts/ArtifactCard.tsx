import { type ReactNode, useRef } from "react";

import { InfoCircle } from "../ui/InfoCircle";
import styles from "./ArtifactCard.module.css";

interface ArtifactCardProps {
  title: string;
  icon?: ReactNode;
  children: ReactNode;
}

export function ArtifactCard({ title, icon, children }: ArtifactCardProps) {
  const bodyRef = useRef<HTMLDivElement>(null);

  const onCopy = async () => {
    const text = (bodyRef.current?.innerText ?? bodyRef.current?.textContent ?? "").trim();
    if (text.length === 0) {
      return;
    }
    if (typeof navigator.clipboard?.writeText !== "function") {
      return;
    }
    await navigator.clipboard.writeText(text);
  };

  return (
    <section className={styles.card} role="region" aria-label={title}>
      <header className={styles.header}>
        <div className={styles.titleRow}>
          {icon ?? null}
          <h3 className={styles.title}>{title}</h3>
        </div>
        <div className={styles.actions}>
          <button type="button" className={styles.actionButton} onClick={onCopy} aria-label="Copy">
            <InfoCircle tooltip="Copy panel content to clipboard." />
            <span>Copy</span>
          </button>
          <button
            type="button"
            className={styles.actionButton}
            aria-label="Expand"
            aria-disabled="true"
            onClick={(event) => event.preventDefault()}
          >
            <InfoCircle tooltip="Full view coming soon" />
            <span>Expand</span>
          </button>
        </div>
      </header>
      <div ref={bodyRef} className={styles.body}>
        {children}
      </div>
    </section>
  );
}

export type { ArtifactCardProps };
