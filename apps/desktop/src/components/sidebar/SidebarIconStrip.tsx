import { InfoCircle } from "../ui/InfoCircle";
import styles from "./SidebarIconStrip.module.css";

export type EnvelopeAlert = "none" | "caution" | "danger";

interface SidebarIconStripProps {
  envelopeAlert: EnvelopeAlert;
  hasPending: boolean;
  onToggle: () => void;
}

function IconTile({ label, icon }: { label: string; icon: string }) {
  return (
    <div className={styles.tile} aria-label={label}>
      <span aria-hidden="true">{icon}</span>
    </div>
  );
}

export function SidebarIconStrip({ envelopeAlert, hasPending, onToggle }: SidebarIconStripProps) {
  return (
    <div className={styles.strip} aria-label="Sidebar icon strip">
      <div className={styles.toggle}>
        <InfoCircle
          onClick={onToggle}
          aria-label="Hide sidebar"
          tooltip="Hide sidebar (Cmd/Ctrl+B)"
        />
      </div>

      <IconTile label="Accounts" icon="A" />

      <div className={styles.alertTile}>
        <IconTile label="Envelopes" icon="E" />
        {envelopeAlert === "danger" ? (
          <span className={`${styles.dot} ${styles.danger}`} aria-label="Envelopes alert" />
        ) : null}
        {envelopeAlert === "caution" ? (
          <span className={`${styles.dot} ${styles.caution}`} aria-label="Envelopes alert" />
        ) : null}
      </div>

      <div className={styles.alertTile}>
        <IconTile label="Coming up" icon="C" />
        {hasPending ? <span className={`${styles.dot} ${styles.info}`} aria-label="Coming up alert" /> : null}
      </div>
    </div>
  );
}
