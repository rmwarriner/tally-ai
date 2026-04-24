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
}

const ICON: Record<SetupCardVariant, string> = {
  household_created: "🏠",
  account_created: "🏦",
  opening_balance: "💰",
  envelope_created: "📋",
  gnucash_file_picker: "📂",
};

export function SetupCard({ variant, title, detail }: SetupCardProps) {
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
