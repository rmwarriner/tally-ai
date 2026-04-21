import { useEnvelopeStatuses, usePendingTransactions } from "../../hooks/useSidebarData";
import type { SidebarState } from "../../stores/uiStore";
import { AccountsPanel } from "./AccountsPanel";
import { ComingUpPanel } from "./ComingUpPanel";
import { EnvelopesPanel } from "./EnvelopesPanel";
import { SidebarIconStrip } from "./SidebarIconStrip";
import { SidebarToggle } from "./SidebarToggle";
import styles from "./HealthSidebar.module.css";

interface HealthSidebarProps {
  state: SidebarState;
  onToggle: () => void;
}

function sidebarWidth(state: SidebarState): string {
  if (state === "open") {
    return "280px";
  }
  if (state === "icon") {
    return "48px";
  }
  return "0px";
}

function getEnvelopeAlert(envelopeData: Array<{ allocated_cents: number; spent_cents: number }>): "none" | "caution" | "danger" {
  const percentages = envelopeData.map((envelope) => {
    if (envelope.allocated_cents <= 0) {
      return 0;
    }
    return (envelope.spent_cents / envelope.allocated_cents) * 100;
  });

  if (percentages.some((percentage) => percentage >= 100)) {
    return "danger";
  }

  if (percentages.some((percentage) => percentage >= 80)) {
    return "caution";
  }

  return "none";
}

export function HealthSidebar({ state, onToggle }: HealthSidebarProps) {
  const envelopesQuery = useEnvelopeStatuses();
  const pendingQuery = usePendingTransactions();

  const envelopeAlert = getEnvelopeAlert(envelopesQuery.data ?? []);
  const hasPending = (pendingQuery.data ?? []).length > 0;

  return (
    <aside
      className={styles.sidebar}
      style={{ width: sidebarWidth(state) }}
      aria-label="Financial health"
    >
      {state === "open" ? (
        <>
          <SidebarToggle open onToggle={onToggle} />
          <div className={styles.content}>
            <AccountsPanel />
            <EnvelopesPanel />
            <ComingUpPanel />
          </div>
        </>
      ) : null}

      {state === "icon" ? (
        <SidebarIconStrip
          envelopeAlert={envelopeAlert}
          hasPending={hasPending}
          onToggle={onToggle}
        />
      ) : null}
    </aside>
  );
}
