import type { ContextChip as ContextChipType } from "../../stores/uiStore";
import styles from "./ContextChip.module.css";

interface ContextChipProps {
  chip: ContextChipType;
  onRemove: (id: string) => void;
}

export function ContextChip({ chip, onRemove }: ContextChipProps) {
  const variantClassName =
    chip.type === "account"
      ? styles.account
      : chip.type === "envelope"
        ? styles.envelope
        : styles.dateRange;

  return (
    <span className={`${styles.chip} ${variantClassName}`}>
      <span>{chip.label}</span>
      <button
        type="button"
        className={styles.dismiss}
        aria-label={`Remove ${chip.label} filter`}
        onClick={() => onRemove(chip.id)}
      >
        ×
      </button>
    </span>
  );
}
