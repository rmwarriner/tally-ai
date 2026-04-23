import type { SlashCommand } from "./SLASH_COMMANDS";
import styles from "./SlashPalette.module.css";

interface SlashPaletteProps {
  commands: readonly SlashCommand[];
  selectedIndex: number;
  onSelect: (commandName: string) => void;
  onHover: (index: number) => void;
}

export function SlashPalette({ commands, selectedIndex, onSelect, onHover }: SlashPaletteProps) {
  return (
    <div className={styles.palette} role="listbox" aria-label="Slash commands">
      {commands.map((command, index) => {
        const selected = index === selectedIndex;
        return (
          <button
            key={command.name}
            type="button"
            role="option"
            aria-selected={selected}
            className={`${styles.option} ${selected ? styles.optionSelected : ""}`.trim()}
            onMouseEnter={() => onHover(index)}
            onMouseDown={(event) => {
              event.preventDefault();
              onSelect(command.name);
            }}
          >
            <span className={styles.name}>{command.name}</span>
            <span className={styles.description}>{command.description}</span>
          </button>
        );
      })}
    </div>
  );
}
