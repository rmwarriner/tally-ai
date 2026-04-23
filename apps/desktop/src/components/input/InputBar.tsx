import { type KeyboardEvent, useMemo, useState } from "react";

import { ChatTextarea } from "./ChatTextarea";
import { ChipStrip } from "./ChipStrip";
import { SLASH_COMMANDS } from "./SLASH_COMMANDS";
import { SlashPalette } from "./SlashPalette";
import styles from "./InputBar.module.css";

interface InputBarProps {
  onSend: (text: string) => void;
  isStreaming: boolean;
}

function isSlashCommandInput(value: string): boolean {
  return value.startsWith("/") && !value.includes(" ");
}

export function InputBar({ onSend, isStreaming }: InputBarProps) {
  const [value, setValue] = useState("");
  const [isPaletteOpen, setIsPaletteOpen] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const filteredCommands = useMemo(() => {
    if (!value.startsWith("/")) {
      return [];
    }
    const query = value.slice(1).trim().toLowerCase();
    return SLASH_COMMANDS.filter((command) => command.name.slice(1).startsWith(query));
  }, [value]);

  const selectCommand = (commandName: string) => {
    setValue(`${commandName} `);
    setIsPaletteOpen(false);
    setSelectedIndex(0);
  };

  const sendCurrentValue = () => {
    const trimmed = value.trim();
    if (trimmed.length === 0 || isStreaming) {
      return;
    }
    onSend(trimmed);
    setValue("");
    setIsPaletteOpen(false);
    setSelectedIndex(0);
  };

  const onTextareaKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      if (isPaletteOpen) {
        setIsPaletteOpen(false);
        return;
      }
      if (value.length > 0) {
        setValue("");
      }
      return;
    }

    if (event.key === "ArrowDown" && isPaletteOpen && filteredCommands.length > 0) {
      event.preventDefault();
      setSelectedIndex((current) => (current + 1) % filteredCommands.length);
      return;
    }

    if (event.key === "ArrowUp" && isPaletteOpen && filteredCommands.length > 0) {
      event.preventDefault();
      setSelectedIndex((current) =>
        current === 0 ? filteredCommands.length - 1 : current - 1,
      );
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (isPaletteOpen && filteredCommands.length > 0) {
        selectCommand(filteredCommands[selectedIndex]?.name ?? filteredCommands[0].name);
        return;
      }
      sendCurrentValue();
      return;
    }

    if (event.key === "Enter" && event.shiftKey) {
      event.preventDefault();
      setValue((current) => `${current}\n`);
    }
  };

  return (
    <div className={styles.shell}>
      <ChipStrip />
      <div className={styles.row}>
        <div>
          <ChatTextarea
            value={value}
            disabled={isStreaming}
            onChange={(event) => {
              const nextValue = event.target.value;
              setValue(nextValue);

              if (isSlashCommandInput(nextValue)) {
                setIsPaletteOpen(true);
                setSelectedIndex(0);
              } else {
                setIsPaletteOpen(false);
              }
            }}
            onBlur={() => {
              setTimeout(() => setIsPaletteOpen(false), 0);
            }}
            onKeyDown={onTextareaKeyDown}
          />
          {isPaletteOpen && filteredCommands.length > 0 ? (
            <SlashPalette
              commands={filteredCommands}
              selectedIndex={Math.min(selectedIndex, filteredCommands.length - 1)}
              onHover={setSelectedIndex}
              onSelect={selectCommand}
            />
          ) : null}
        </div>

        <button
          type="button"
          className={styles.sendButton}
          aria-label="Send message"
          disabled={value.trim().length === 0 || isStreaming}
          onClick={sendCurrentValue}
          onKeyDown={(event) => {
            if (event.key === " ") {
              event.preventDefault();
            }
          }}
        >
          Send
        </button>
      </div>
    </div>
  );
}

export type { InputBarProps };
