import { useUIStore } from "../../stores/uiStore";
import { ContextChip } from "./ContextChip";
import styles from "./ChipStrip.module.css";

export function ChipStrip() {
  const chips = useUIStore((state) => state.contextChips);
  const removeContextChip = useUIStore((state) => state.removeContextChip);

  if (chips.length === 0) {
    return null;
  }

  return (
    <div className={styles.strip}>
      {chips.map((chip) => (
        <ContextChip key={chip.id} chip={chip} onRemove={removeContextChip} />
      ))}
    </div>
  );
}
