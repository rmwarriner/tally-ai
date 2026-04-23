import type { TransactionDisplay } from "./TransactionCard.types";
import { TransactionCardPosted } from "./TransactionCardPosted";
import { TransactionCardVoided } from "./TransactionCardVoided";
import styles from "./TransactionCard.module.css";

interface TransactionCardCorrectionPairProps {
  transaction: TransactionDisplay;
  replacement: TransactionDisplay;
}

export function TransactionCardCorrectionPair({
  transaction,
  replacement,
}: TransactionCardCorrectionPairProps) {
  return (
    <article role="article" aria-label={`Correction: ${transaction.payee}`}>
      <TransactionCardVoided transaction={transaction} as="div" />
      <div className={styles.connector}>
        <span className={styles.connectorLabel}>corrected ↓</span>
      </div>
      <TransactionCardPosted transaction={replacement} as="div" />
    </article>
  );
}
