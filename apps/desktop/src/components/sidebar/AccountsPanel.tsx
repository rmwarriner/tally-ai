import { useAccountBalances } from "../../hooks/useSidebarData";
import { formatCents } from "../../utils/formatCents";
import { SidebarPanel } from "./SidebarPanel";
import styles from "./AccountsPanel.module.css";

export function AccountsPanel() {
  const { data, isLoading, error } = useAccountBalances();

  const accounts = (data ?? []).filter((account) => account.type === "asset" || account.type === "liability");

  return (
    <SidebarPanel
      title="Account balances"
      isLoading={isLoading}
      error={Boolean(error)}
      isEmpty={accounts.length === 0}
      emptyMessage="No accounts yet"
    >
      <ul className={styles.list}>
        {accounts.map((account) => (
          <li className={styles.row} key={account.id}>
            <span className={styles.name} title={account.name}>
              {account.name}
            </span>
            <span
              className={`${styles.amount} ${account.type === "liability" && account.balance_cents > 0 ? styles.liability : ""}`.trim()}
            >
              {formatCents(account.balance_cents)}
            </span>
          </li>
        ))}
      </ul>
    </SidebarPanel>
  );
}
