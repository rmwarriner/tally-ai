import { formatCents } from "../../utils/formatCents";
import { JournalLinesDrawer } from "./JournalLinesDrawer";
import type { TransactionDisplay } from "./TransactionCard.types";
import { formatTransactionAriaLabel, formatTransactionDate } from "./transactionCardFormat";
import styles from "./TransactionCard.module.css";

interface TransactionCardPostedProps {
  transaction: TransactionDisplay;
  as?: "article" | "div";
  ariaLabel?: string;
}

export function TransactionCardPosted({
  transaction,
  as = "article",
  ariaLabel,
}: TransactionCardPostedProps) {
  const label = ariaLabel ?? formatTransactionAriaLabel(transaction.payee, transaction.amount_cents);
  const content = (
    <div className={styles.content}>
      <div className={styles.header}>
        <div className={styles.meta}>
          <p className={styles.payee}>{transaction.payee}</p>
          <div className={styles.dateAccount}>
            {formatTransactionDate(transaction.txn_date)} • {transaction.account_name}
          </div>
        </div>
        <p className={styles.amount}>{formatCents(transaction.amount_cents)}</p>
      </div>
      <JournalLinesDrawer lines={transaction.lines} />
    </div>
  );

  if (as === "div") {
    return <div className={`${styles.card} ${styles.posted}`}>{content}</div>;
  }

  return (
    <article className={`${styles.card} ${styles.posted}`} role="article" aria-label={label}>
      {content}
    </article>
  );
}
