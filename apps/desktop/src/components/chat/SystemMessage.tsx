import styles from "./SystemMessage.module.css";

interface SystemMessageProps {
  text: string;
  tone?: "info" | "error";
}

export function SystemMessage({ text, tone = "info" }: SystemMessageProps) {
  return <p className={`${styles.message} ${tone === "error" ? styles.error : ""}`.trim()}>{text}</p>;
}
