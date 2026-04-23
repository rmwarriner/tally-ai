import { formatCents } from "../../utils/formatCents";
import type { BalanceNode } from "./artifactTypes";
import styles from "./BalanceReport.module.css";

interface BalanceReportProps {
  nodes: BalanceNode[];
}

export function BalanceReport({ nodes }: BalanceReportProps) {
  return (
    <ul className={styles.list}>
      {nodes.map((node) => {
        const rowClassName = `${styles.row} ${node.is_subtotal ? styles.subtotal : ""}`.trim();
        const amountClassName = `${styles.amount} ${node.balance_cents < 0 ? styles.negative : ""}`.trim();

        return (
          <li key={`${node.account_name}-${node.depth}-${node.balance_cents}`} className={rowClassName}>
            <span className={styles.name} style={{ paddingLeft: `${node.depth * 16}px` }}>
              {node.account_name}
            </span>
            <span className={amountClassName}>{formatCents(node.balance_cents)}</span>
          </li>
        );
      })}
    </ul>
  );
}

export type { BalanceReportProps };
