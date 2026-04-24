import type { ImportAccountType, ImportPlan } from "@tally/core-types";

import styles from "./GnuCashMappingCard.module.css";

interface GnuCashMappingCardProps {
  plan: ImportPlan;
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

export function GnuCashMappingCard({ plan, onConfirm, onRequestEdit }: GnuCashMappingCardProps) {
  const accountCount = plan.account_mappings.length;
  const transactionCount = plan.transactions.length;

  return (
    <div className={styles.card} role="region" aria-label="GnuCash account mapping">
      <div className={styles.header}>
        <span className={styles.stat}>
          <strong>{accountCount}</strong> {accountCount === 1 ? "account" : "accounts"}
        </span>
        <span className={styles.statSep}>·</span>
        <span className={styles.stat}>
          <strong>{transactionCount}</strong>{" "}
          {transactionCount === 1 ? "transaction" : "transactions"}
        </span>
      </div>

      <table className={styles.table}>
        <thead>
          <tr>
            <th className={styles.th}>GnuCash account</th>
            <th className={styles.th}>Tally type</th>
          </tr>
        </thead>
        <tbody>
          {plan.account_mappings.map((mapping) => (
            <tr key={mapping.gnc_guid} className={styles.row}>
              <td className={styles.td}>{mapping.gnc_full_name}</td>
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

export type { GnuCashMappingCardProps };
