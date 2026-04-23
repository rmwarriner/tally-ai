import { formatCents } from "../../utils/formatCents";
import type { LedgerRow } from "./artifactTypes";
import styles from "./LedgerTable.module.css";

const DATE_FORMAT = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
});

interface LedgerTableProps {
  rows: LedgerRow[];
}

function formatSignedAmount(row: LedgerRow): string {
  return row.side === "credit" ? `-${formatCents(row.amount_cents)}` : formatCents(row.amount_cents);
}

export function LedgerTable({ rows }: LedgerTableProps) {
  const sortedRows = [...rows].sort((a, b) => b.date - a.date);

  return (
    <table className={styles.table}>
      <tbody>
        {sortedRows.map((row) => (
          <tr key={`${row.date}-${row.payee}-${row.amount_cents}`} className={styles.row}>
            <td className={styles.cell}>
              <div className={styles.meta}>
                <span className={styles.date}>{DATE_FORMAT.format(new Date(row.date))}</span>
                <span className={styles.payee}>{row.payee}</span>
              </div>
            </td>
            <td className={`${styles.cell} ${styles.amount}`}>{formatSignedAmount(row)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export type { LedgerTableProps };
