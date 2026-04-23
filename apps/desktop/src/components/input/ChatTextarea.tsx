import { type KeyboardEventHandler, type ChangeEventHandler, useEffect, useRef } from "react";

import styles from "./ChatTextarea.module.css";

const MAX_HEIGHT_PX = 144;

interface ChatTextareaProps {
  value: string;
  onChange: ChangeEventHandler<HTMLTextAreaElement>;
  onKeyDown: KeyboardEventHandler<HTMLTextAreaElement>;
  onBlur: () => void;
  disabled: boolean;
}

export function ChatTextarea({ value, onChange, onKeyDown, onBlur, disabled }: ChatTextareaProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea === null) {
      return;
    }

    textarea.style.height = "auto";
    textarea.style.height = `${Math.min(textarea.scrollHeight, MAX_HEIGHT_PX)}px`;
    textarea.style.overflowY = textarea.scrollHeight > MAX_HEIGHT_PX ? "auto" : "hidden";
  }, [value]);

  return (
    <textarea
      ref={textareaRef}
      className={`${styles.textarea} ${disabled ? styles.disabled : ""}`.trim()}
      placeholder="Ask anything, or type / for commands"
      aria-label="Chat input"
      aria-multiline="true"
      value={value}
      onChange={onChange}
      onKeyDown={onKeyDown}
      onBlur={onBlur}
      disabled={disabled}
      rows={1}
    />
  );
}
