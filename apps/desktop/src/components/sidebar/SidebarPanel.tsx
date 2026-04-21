import styles from "./SidebarPanel.module.css";

interface SidebarPanelProps {
  title: string;
  isLoading: boolean;
  error: boolean;
  isEmpty: boolean;
  emptyMessage: string;
  children: React.ReactNode;
}

export function SidebarPanel({
  title,
  isLoading,
  error,
  isEmpty,
  emptyMessage,
  children,
}: SidebarPanelProps) {
  return (
    <section className={styles.panel}>
      <h2 className={styles.header}>{title}</h2>
      {isLoading ? <p className={styles.state}>Loading…</p> : null}
      {!isLoading && error ? <p className={styles.state}>Could not load this section.</p> : null}
      {!isLoading && !error && isEmpty ? <p className={styles.state}>{emptyMessage}</p> : null}
      {!isLoading && !error && !isEmpty ? <div className={styles.body}>{children}</div> : null}
    </section>
  );
}
