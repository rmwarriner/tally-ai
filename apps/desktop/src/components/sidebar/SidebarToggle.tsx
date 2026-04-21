import { InfoCircle } from "../ui/InfoCircle";
import styles from "./SidebarToggle.module.css";

interface SidebarToggleProps {
  open: boolean;
  onToggle: () => void;
}

export function SidebarToggle({ open, onToggle }: SidebarToggleProps) {
  const ariaLabel = open ? "Collapse sidebar" : "Expand sidebar";
  const shortcut = navigator.userAgent.includes("Mac") ? "Cmd+B" : "Ctrl+B";

  return (
    <div className={styles.container}>
      <InfoCircle
        onClick={onToggle}
        aria-label={ariaLabel}
        tooltip={`${ariaLabel} (${shortcut})`}
        className={styles.info}
      />
    </div>
  );
}
