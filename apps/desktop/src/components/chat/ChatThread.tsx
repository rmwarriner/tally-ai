import styles from "./ChatThread.module.css";

export function ChatThread() {
  return (
    <section className={styles.thread} role="log" aria-label="Chat thread" aria-live="polite">
      <p className={styles.placeholder}>Chat thread</p>
    </section>
  );
}
