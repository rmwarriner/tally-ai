import { InfoCircle } from "../ui/InfoCircle";
import { formatCents } from "../../utils/formatCents";
import { JournalLinesDrawer } from "./JournalLinesDrawer";
import type { TransactionDisplay } from "./TransactionCard.types";
import { formatTransactionAriaLabel, formatTransactionDate } from "./transactionCardFormat";
import styles from "./TransactionCard.module.css";

interface TransactionCardPendingProps {
  transaction: TransactionDisplay;
  onSendMessage?: (message: string) => void;
}

export function TransactionCardPending({ transaction, onSendMessage }: TransactionCardPendingProps) {
  return (
    <article
      className={`${styles.card} ${styles.pending}`}
      role="article"
      aria-label={formatTransactionAriaLabel(transaction.payee, transaction.amount_cents)}
    >
      <div className={styles.content}>
        <div className={styles.header}>
          <div className={styles.meta}>
            <p className={styles.payee}>{transaction.payee}</p>
            <div className={styles.dateAccount}>
              {formatTransactionDate(transaction.txn_date)} • {transaction.account_name}
            </div>
            <span className={`${styles.badge} ${styles.badgePending}`}>Pending</span>
          </div>
          <p className={styles.amount}>{formatCents(transaction.amount_cents)}</p>
        </div>

        <div className={styles.actionRow}>
          <button
            type="button"
            className={styles.postNowButton}
            onClick={() => onSendMessage?.(`/fix post ${transaction.id}`)}
          >
            <InfoCircle tooltip="Send a post command for this transaction." />
            <span>Post now</span>
          </button>
        </div>

        <JournalLinesDrawer lines={transaction.lines} />
      </div>
    </article>
  );
}
