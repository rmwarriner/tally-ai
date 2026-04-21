import { useEnvelopeStatuses } from "../../hooks/useSidebarData";
import { formatCents } from "../../utils/formatCents";
import { SidebarPanel } from "./SidebarPanel";
import styles from "./EnvelopesPanel.module.css";

function getUsagePercent(spentCents: number, allocatedCents: number): number {
  if (allocatedCents <= 0) {
    return 0;
  }
  return (spentCents / allocatedCents) * 100;
}

function getUsageColor(percent: number): string {
  if (percent >= 100) {
    return "var(--color-danger)";
  }
  if (percent >= 80) {
    return "var(--color-caution)";
  }
  return "var(--color-positive)";
}

export function EnvelopesPanel() {
  const { data, isLoading, error } = useEnvelopeStatuses();
  const envelopes = data ?? [];

  return (
    <SidebarPanel
      title="Envelopes"
      isLoading={isLoading}
      error={Boolean(error)}
      isEmpty={envelopes.length === 0}
      emptyMessage="No envelopes this month"
    >
      <ul className={styles.list}>
        {envelopes.map((envelope) => {
          const percent = getUsagePercent(envelope.spent_cents, envelope.allocated_cents);
          const overAmount = envelope.spent_cents - envelope.allocated_cents;
          const isOver = overAmount > 0;

          return (
            <li className={styles.row} key={envelope.envelope_id}>
              <div className={styles.rowHeader}>
                <span className={styles.name} title={envelope.name}>
                  {envelope.name}
                </span>
                <span className={`${styles.label} ${isOver ? styles.over : ""}`.trim()}>
                  {isOver
                    ? `${formatCents(overAmount)} over`
                    : `${formatCents(envelope.spent_cents)} / ${formatCents(envelope.allocated_cents)}`}
                </span>
              </div>
              <div className={styles.track}>
                <div
                  className={styles.fill}
                  role="progressbar"
                  aria-label={`${envelope.name} ${Math.round(percent)}% used`}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(percent)}
                  style={{
                    width: `${Math.min(Math.max(percent, 0), 100)}%`,
                    backgroundColor: getUsageColor(percent),
                  }}
                />
              </div>
            </li>
          );
        })}
      </ul>
    </SidebarPanel>
  );
}
