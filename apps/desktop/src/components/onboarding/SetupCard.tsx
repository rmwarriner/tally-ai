import { type FormEvent, useState } from "react";

import styles from "./SetupCard.module.css";

export type SetupCardVariant =
  | "household_created"
  | "account_created"
  | "opening_balance"
  | "envelope_created"
  | "gnucash_file_picker";

export interface SetupCardProps {
  variant: SetupCardVariant;
  title: string;
  detail: string;
  onSubmitGnuCashPath?: (path: string) => void;
}

const ICON: Record<SetupCardVariant, string> = {
  household_created: "🏠",
  account_created: "🏦",
  opening_balance: "💰",
  envelope_created: "📋",
  gnucash_file_picker: "📂",
};

function GnuCashFilePickerCard({ onSubmitGnuCashPath }: { onSubmitGnuCashPath?: (path: string) => void }) {
  const [path, setPath] = useState("");

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (path.trim()) {
      onSubmitGnuCashPath?.(path.trim());
    }
  };

  return (
    <form onSubmit={onSubmit} className={styles.gnucashPicker}>
      <label htmlFor="gnucash-path">Path to your GnuCash file</label>
      <input
        id="gnucash-path"
        type="text"
        value={path}
        onChange={e => setPath(e.target.value)}
        placeholder="/Users/you/Documents/household.gnucash"
        aria-describedby="gnucash-path-hint"
      />
      <p id="gnucash-path-hint" className={styles.hint}>
        Save your GnuCash book with File → Save As → SQLite, then paste the full path here.
      </p>
      <button type="submit" disabled={!path.trim()}>Use this file</button>
    </form>
  );
}

export function SetupCard({ variant, title, detail, onSubmitGnuCashPath }: SetupCardProps) {
  if (variant === "gnucash_file_picker") {
    return (
      <div className={`${styles.card} ${styles[variant]}`}>
        <GnuCashFilePickerCard onSubmitGnuCashPath={onSubmitGnuCashPath} />
      </div>
    );
  }

  return (
    <div className={`${styles.card} ${styles[variant]}`} role="status">
      <span className={styles.icon} aria-label="created">
        {ICON[variant]}
      </span>
      <div className={styles.body}>
        <span className={styles.title}>{title}</span>
        <span className={styles.detail}>{detail}</span>
      </div>
    </div>
  );
}
