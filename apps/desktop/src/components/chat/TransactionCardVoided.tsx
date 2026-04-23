import { formatCents } from "../../utils/formatCents";
import { JournalLinesDrawer } from "./JournalLinesDrawer";
import type { TransactionDisplay } from "./TransactionCard.types";
import { formatTransactionAriaLabel, formatTransactionDate } from "./transactionCardFormat";
import styles from "./TransactionCard.module.css";

interface TransactionCardVoidedProps {
  transaction: TransactionDisplay;
  as?: "article" | "div";
  ariaLabel?: string;
}

export function TransactionCardVoided({
  transaction,
  as = "article",
  ariaLabel,
}: TransactionCardVoidedProps) {
  const label = ariaLabel ?? formatTransactionAriaLabel(transaction.payee, transaction.amount_cents);
  const content = (
    <div className={styles.content}>
      <div className={styles.header}>
        <div className={styles.meta}>
          <p className={`${styles.payee} ${styles.struck}`}>{transaction.payee}</p>
          <div className={styles.dateAccount}>
            {formatTransactionDate(transaction.txn_date)} • {transaction.account_name}
          </div>
          <span className={`${styles.badge} ${styles.badgeVoided}`}>Voided</span>
        </div>
        <p className={`${styles.amount} ${styles.struck}`}>{formatCents(transaction.amount_cents)}</p>
      </div>
      <JournalLinesDrawer lines={transaction.lines} />
    </div>
  );

  if (as === "div") {
    return <div className={`${styles.card} ${styles.voided}`}>{content}</div>;
  }

  return (
    <article className={`${styles.card} ${styles.voided}`} role="article" aria-label={label}>
      {content}
    </article>
  );
}
