import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useUIStore } from "../../stores/uiStore";
import { InputBar } from "./InputBar";

describe("InputBar", () => {
  it("sends on Enter and clears input", () => {
    const onSend = vi.fn();
    render(<InputBar onSend={onSend} isStreaming={false} />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "Hello" } });
    fireEvent.keyDown(textbox, { key: "Enter" });

    expect(onSend).toHaveBeenCalledWith("Hello");
    expect(textbox).toHaveValue("");
  });

  it("Shift+Enter inserts newline instead of sending", () => {
    const onSend = vi.fn();
    render(<InputBar onSend={onSend} isStreaming={false} />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "Line1" } });
    fireEvent.keyDown(textbox, { key: "Enter", shiftKey: true });

    expect(onSend).not.toHaveBeenCalled();
    expect(textbox).toHaveValue("Line1\n");
  });

  it("opens slash palette on / at start of empty input", () => {
    render(<InputBar onSend={vi.fn()} isStreaming={false} />);

    fireEvent.change(screen.getByRole("textbox", { name: /chat input/i }), {
      target: { value: "/" },
    });
    expect(screen.getByRole("listbox", { name: /slash commands/i })).toBeInTheDocument();
  });

  it("filters slash palette items as user types", () => {
    render(<InputBar onSend={vi.fn()} isStreaming={false} />);

    fireEvent.change(screen.getByRole("textbox", { name: /chat input/i }), {
      target: { value: "/b" },
    });

    const options = screen.getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(
      expect.arrayContaining([expect.stringContaining("/budget"), expect.stringContaining("/balance")]),
    );
    expect(options.some((option) => option.textContent?.includes("/recent"))).toBe(false);
  });

  it("shows context chips from the ui store and allows removal", () => {
    useUIStore.setState({
      contextChips: [
        { id: "chip-1", type: "account", label: "Checking" },
        { id: "chip-2", type: "envelope", label: "Groceries" },
      ],
    });

    render(<InputBar onSend={vi.fn()} isStreaming={false} />);

    expect(screen.getByText("Checking")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /remove checking filter/i }));

    expect(useUIStore.getState().contextChips).toEqual([
      { id: "chip-2", type: "envelope", label: "Groceries" },
    ]);

    useUIStore.setState({ contextChips: [] });
  });
});
