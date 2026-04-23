import { useState } from "react";

import { InfoCircle } from "../ui/InfoCircle";
import { formatCents } from "../../utils/formatCents";
import type { JournalLineDisplay } from "./TransactionCard.types";
import styles from "./TransactionCard.module.css";

interface JournalLinesDrawerProps {
  lines: JournalLineDisplay[];
}

function formatLineLabel(line: JournalLineDisplay): string {
  return line.envelope_name ? `${line.account_name} / ${line.envelope_name}` : line.account_name;
}

export function JournalLinesDrawer({ lines }: JournalLinesDrawerProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={styles.drawer}>
      <button
        type="button"
        className={styles.drawerToggle}
        aria-expanded={expanded}
        onClick={() => setExpanded((current) => !current)}
      >
        <span className={styles.drawerLabel}>
          {expanded ? "Hide journal lines" : "Show journal lines"}
          <InfoCircle tooltip="View debit and credit journal entries." />
        </span>
      </button>

      {expanded ? (
        <ul className={styles.lineList}>
          {lines.length === 0 ? (
            <li className={styles.lineItem}>
              <span className={styles.lineAccount}>No journal lines</span>
            </li>
          ) : (
            lines.map((line, index) => (
              <li key={`${line.account_name}-${line.side}-${index}`} className={styles.lineItem}>
                <span className={styles.side}>{line.side}</span>
                <span className={styles.lineAccount}>{formatLineLabel(line)}</span>
                <span className={styles.lineAmount}>{formatCents(line.amount_cents)}</span>
              </li>
            ))
          )}
        </ul>
      ) : null}
    </div>
  );
}
