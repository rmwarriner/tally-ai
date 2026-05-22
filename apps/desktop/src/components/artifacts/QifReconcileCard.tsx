import type { QifBalanceReportArtifact } from "@tally/core-types";

import styles from "./GnuCashReconcileCard.module.css";

interface Props {
  report: QifBalanceReportArtifact;
  onAccept: () => void;
  onRollback: () => void;
}

function formatCents(n: number): string {
  const abs = Math.abs(n);
  const dollars = (abs / 100).toFixed(2);
  return `${n < 0 ? "-" : ""}$${dollars}`;
}

export function QifReconcileCard({ report, onAccept, onRollback }: Props) {
  const { rows, total_mismatches } = report;
  const headline =
    total_mismatches === 0
      ? "All balances match the QIF file."
      : `${total_mismatches} mismatch${total_mismatches === 1 ? "" : "es"} — review below.`;

  return (
    <div className={styles.card} role="region" aria-label="QIF reconcile report">
      <div className={styles.header}>
        <h3 className={styles.title}>Balance reconciliation</h3>
        <p className={styles.headline}>{headline}</p>
      </div>
      <table className={styles.table}>
        <thead>
          <tr>
            <th className={styles.th}>Account</th>
            <th className={styles.th}>Tally</th>
            <th className={styles.th}>QIF declared</th>
            <th className={styles.th} aria-label="status"></th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr
              key={r.account_name}
              className={`${styles.row} ${r.matches ? "" : styles.mismatchRow}`}
            >
              <td className={styles.td}>{r.account_name}</td>
              <td className={styles.td}>{formatCents(r.tally_cents)}</td>
              <td className={styles.td}>{formatCents(r.declared_cents)}</td>
              <td className={`${styles.td} ${styles.statusCell}`}>
                {r.matches ? "✓" : "!"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className={styles.actions}>
        <button
          type="button"
          className={`${styles.btn} ${styles.btnAccept}`}
          onClick={onAccept}
        >
          Looks right, continue
        </button>
        <button
          type="button"
          className={`${styles.btn} ${styles.btnRollback}`}
          onClick={onRollback}
        >
          Roll back
        </button>
      </div>
    </div>
  );
}
