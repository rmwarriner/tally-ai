import type { ImportAccountType, QifImportPlan } from "@tally/core-types";

import styles from "./GnuCashMappingCard.module.css";

interface QifMappingCardProps {
  plan: QifImportPlan;
  skippedSecurityTrades: number;
  onConfirm: () => void;
  onRequestEdit: () => void;
}

const TYPE_PILL_CLASS: Record<ImportAccountType, string> = {
  asset: styles.pillAsset,
  liability: styles.pillLiability,
  income: styles.pillIncome,
  expense: styles.pillExpense,
  equity: styles.pillEquity,
};

export function QifMappingCard({
  plan,
  skippedSecurityTrades,
  onConfirm,
  onRequestEdit,
}: QifMappingCardProps) {
  const accountCount = plan.account_mappings.length;
  const transactionCount = plan.transactions.length;

  return (
    <div className={styles.card} role="region" aria-label="QIF account mapping">
      <div className={styles.header}>
        <span className={styles.stat}>
          <strong>{accountCount}</strong> {accountCount === 1 ? "account" : "accounts"}
        </span>
        <span className={styles.statSep}>·</span>
        <span className={styles.stat}>
          <strong>{transactionCount}</strong>{" "}
          {transactionCount === 1 ? "transaction" : "transactions"}
        </span>
        {skippedSecurityTrades > 0 && (
          <>
            <span className={styles.statSep}>·</span>
            <span className={styles.stat}>
              <strong>{skippedSecurityTrades}</strong> security{" "}
              {skippedSecurityTrades === 1 ? "trade" : "trades"} skipped
            </span>
          </>
        )}
      </div>

      <table className={styles.table}>
        <thead>
          <tr>
            <th className={styles.th}>QIF account</th>
            <th className={styles.th}>Tally type</th>
          </tr>
        </thead>
        <tbody>
          {plan.account_mappings.map((mapping) => (
            <tr key={mapping.tally_account_id} className={styles.row}>
              <td className={styles.td}>{mapping.tally_name}</td>
              <td className={styles.td}>
                <span className={`${styles.pill} ${TYPE_PILL_CLASS[mapping.tally_type]}`}>
                  {mapping.tally_type}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <div className={styles.actions}>
        <button
          type="button"
          className={`${styles.btn} ${styles.btnConfirm}`}
          onClick={onConfirm}
        >
          Looks right
        </button>
        <button
          type="button"
          className={`${styles.btn} ${styles.btnEdit}`}
          onClick={onRequestEdit}
        >
          I need to change something
        </button>
      </div>
    </div>
  );
}

export type { QifMappingCardProps };
