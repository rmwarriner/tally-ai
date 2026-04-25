import styles from "./GnuCashReconcileCard.module.css";

interface BalanceRow {
  account_name: string;
  tally_cents: number;
  gnucash_cents: number;
  matches: boolean;
}

export interface GnuCashReconcileReport {
  rows: BalanceRow[];
  total_mismatches: number;
}

interface Props {
  report: GnuCashReconcileReport;
  onAccept: () => void;
  onRollback: () => void;
}

function formatCents(n: number): string {
  const abs = Math.abs(n);
  const dollars = (abs / 100).toFixed(2);
  return `${n < 0 ? "-" : ""}$${dollars}`;
}

export function GnuCashReconcileCard({ report, onAccept, onRollback }: Props) {
  const { rows, total_mismatches } = report;
  const headline =
    total_mismatches === 0
      ? "All balances match GnuCash."
      : `${total_mismatches} mismatch${total_mismatches === 1 ? "" : "es"} — review below.`;

  return (
    <div className={styles.card} role="region" aria-label="GnuCash reconcile report">
      <div className={styles.header}>
        <h3 className={styles.title}>Balance reconciliation</h3>
        <p className={styles.headline}>{headline}</p>
      </div>
      <table className={styles.table}>
        <thead>
          <tr>
            <th className={styles.th}>Account</th>
            <th className={styles.th}>Tally</th>
            <th className={styles.th}>GnuCash</th>
            <th className={styles.th} aria-label="status"></th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.account_name} className={`${styles.row} ${r.matches ? "" : styles.mismatchRow}`}>
              <td className={styles.td}>{r.account_name}</td>
              <td className={styles.td}>{formatCents(r.tally_cents)}</td>
              <td className={styles.td}>{formatCents(r.gnucash_cents)}</td>
              <td className={`${styles.td} ${styles.statusCell}`}>{r.matches ? "✓" : "!"}</td>
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
