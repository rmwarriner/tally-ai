import styles from "./AIAvatar.module.css";

interface AIAvatarProps {
  variant: "standard" | "proactive";
}

export function AIAvatar({ variant }: AIAvatarProps) {
  if (variant === "proactive") {
    return (
      <span className={`${styles.avatar} ${styles.proactive}`} aria-label="Proactive avatar">
        ⚡
      </span>
    );
  }

  return (
    <span className={`${styles.avatar} ${styles.standard}`} aria-label="AI avatar">
      AI
    </span>
  );
}
