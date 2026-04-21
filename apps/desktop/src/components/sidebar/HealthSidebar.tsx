import { SidebarToggle } from "./SidebarToggle";
import styles from "./HealthSidebar.module.css";

interface HealthSidebarProps {
  open: boolean;
  onToggle: () => void;
}

export function HealthSidebar({ open, onToggle }: HealthSidebarProps) {
  return (
    <aside
      className={styles.sidebar}
      style={{ width: open ? "280px" : "0px" }}
      aria-label="Financial health"
    >
      <SidebarToggle open={open} onToggle={onToggle} />
      {open ? (
        <div className={styles.content}>
          <p className={styles.placeholder}>Health sidebar</p>
        </div>
      ) : null}
    </aside>
  );
}
