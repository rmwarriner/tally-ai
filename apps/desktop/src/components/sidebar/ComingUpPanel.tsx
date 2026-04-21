import { usePendingTransactions } from "../../hooks/useSidebarData";
import { formatCents } from "../../utils/formatCents";
import { SidebarPanel } from "./SidebarPanel";
import styles from "./ComingUpPanel.module.css";

const DATE_FORMAT = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
});

function formatDate(unixMs: number): string {
  return DATE_FORMAT.format(new Date(unixMs));
}

export function ComingUpPanel() {
  const { data, isLoading, error } = usePendingTransactions();

  const items = [...(data ?? [])].sort((a, b) => a.txn_date - b.txn_date).slice(0, 5);

  return (
    <SidebarPanel
      title="Coming up"
      isLoading={isLoading}
      error={Boolean(error)}
      isEmpty={items.length === 0}
      emptyMessage="No pending transactions"
    >
      <ul className={styles.list}>
        {items.map((item) => (
          <li className={styles.row} key={item.id}>
            <span className={styles.date}>{formatDate(item.txn_date)}</span>
            <span className={styles.name} title={item.payee ?? item.memo ?? "Untitled transaction"}>
              {item.payee ?? item.memo ?? "Untitled transaction"}
            </span>
            <span className={styles.amount}>{formatCents(item.amount_cents)}</span>
          </li>
        ))}
      </ul>
    </SidebarPanel>
  );
}
