import { InfoCircle } from "../ui/InfoCircle";
import { formatCents } from "../../utils/formatCents";
import { JournalLinesDrawer } from "./JournalLinesDrawer";
import type { TransactionDisplay } from "./TransactionCard.types";
import { formatTransactionAriaLabel, formatTransactionDate } from "./transactionCardFormat";
import styles from "./TransactionCard.module.css";

interface TransactionCardPendingProps {
  transaction: TransactionDisplay;
  onSendMessage?: (message: string) => void;
  /// When provided, the card renders Confirm / Discard actions for a fresh
  /// AI proposal. When omitted, the card falls back to the legacy "Post now"
  /// affordance (scheduled-future transactions, not Phase-1-beta flow).
  onConfirm?: () => void;
  onDiscard?: () => void;
  isCommitting?: boolean;
  commitError?: string;
}

export function TransactionCardPending({
  transaction,
  onSendMessage,
  onConfirm,
  onDiscard,
  isCommitting,
  commitError,
}: TransactionCardPendingProps) {
  const isProposal = onConfirm !== undefined && onDiscard !== undefined;
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
            <span className={`${styles.badge} ${styles.badgePending}`}>
              {isProposal ? "Proposed" : "Pending"}
            </span>
          </div>
          <p className={styles.amount}>{formatCents(transaction.amount_cents)}</p>
        </div>

        {commitError ? (
          <p className={styles.commitError} role="alert">
            {commitError}
          </p>
        ) : null}

        <div className={styles.actionRow}>
          {isProposal ? (
            <>
              <button
                type="button"
                className={styles.postNowButton}
                onClick={onConfirm}
                disabled={isCommitting}
              >
                <span>{isCommitting ? "Saving…" : "Confirm"}</span>
              </button>
              <button
                type="button"
                className={styles.discardButton}
                onClick={onDiscard}
                disabled={isCommitting}
              >
                <span>Discard</span>
              </button>
            </>
          ) : (
            <button
              type="button"
              className={styles.postNowButton}
              onClick={() => onSendMessage?.(`/fix post ${transaction.id}`)}
            >
              <InfoCircle tooltip="Send a post command for this transaction." />
              <span>Post now</span>
            </button>
          )}
        </div>

        <JournalLinesDrawer lines={transaction.lines} />
      </div>
    </article>
  );
}
