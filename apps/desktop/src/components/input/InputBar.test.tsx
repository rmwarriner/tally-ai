import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { checkA11y, expectNoA11yViolations } from "../../test/axe";
import { useUIStore } from "../../stores/uiStore";
import { InputBar } from "./InputBar";

afterEach(() => {
  useUIStore.setState({ contextChips: [] });
});

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

  it("selects highlighted slash command on Enter when palette is open", () => {
    const onSend = vi.fn();
    render(<InputBar onSend={onSend} isStreaming={false} />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "/" } });
    fireEvent.keyDown(textbox, { key: "ArrowDown" });
    fireEvent.keyDown(textbox, { key: "Enter" });

    expect(onSend).not.toHaveBeenCalled();
    expect(textbox).toHaveValue("/balance ");
    expect(screen.queryByRole("listbox", { name: /slash commands/i })).not.toBeInTheDocument();
  });

  it("closes palette on Escape and then clears input on second Escape", async () => {
    render(<InputBar onSend={vi.fn()} isStreaming={false} />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "/" } });
    expect(screen.getByRole("listbox", { name: /slash commands/i })).toBeInTheDocument();

    fireEvent.keyDown(textbox, { key: "Escape" });
    expect(screen.queryByRole("listbox", { name: /slash commands/i })).not.toBeInTheDocument();
    expect(textbox).toHaveValue("/");

    fireEvent.keyDown(textbox, { key: "Escape" });
    expect(textbox).toHaveValue("");

    fireEvent.change(textbox, { target: { value: "/" } });
    fireEvent.blur(textbox);
    await waitFor(() => {
      expect(screen.queryByRole("listbox", { name: /slash commands/i })).not.toBeInTheDocument();
    });
  });

  it("sends from button click and stays disabled while streaming", () => {
    const onSend = vi.fn();
    const { rerender } = render(<InputBar onSend={onSend} isStreaming={false} />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "Clicked" } });
    fireEvent.click(screen.getByRole("button", { name: /send message/i }));
    expect(onSend).toHaveBeenCalledWith("Clicked");

    rerender(<InputBar onSend={onSend} isStreaming={true} />);
    const streamingTextbox = screen.getByRole("textbox", { name: /chat input/i });
    const sendButton = screen.getByRole("button", { name: /send message/i });
    expect(streamingTextbox).toBeDisabled();
    expect(sendButton).toBeDisabled();
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

  it("ArrowUp wraps selection from first to last palette item", () => {
    render(<InputBar onSend={vi.fn()} isStreaming={false} />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "/" } });
    // Initial selectedIndex is 0 (first item). ArrowUp wraps to last.
    fireEvent.keyDown(textbox, { key: "ArrowUp" });
    fireEvent.keyDown(textbox, { key: "Enter" });

    // Last command in SLASH_COMMANDS is /defaults.
    expect(textbox).toHaveValue("/defaults ");
  });

  it("textarea grows with content and respects the max-height cap", () => {
    render(<InputBar onSend={vi.fn()} isStreaming={false} />);
    const textbox = screen.getByRole("textbox", { name: /chat input/i }) as HTMLTextAreaElement;

    // jsdom does not lay out content, so we drive scrollHeight directly to
    // exercise the auto-grow effect in ChatTextarea (MAX_HEIGHT_PX = 144).
    Object.defineProperty(textbox, "scrollHeight", { configurable: true, value: 80 });
    fireEvent.change(textbox, { target: { value: "one\ntwo\nthree" } });
    expect(textbox.style.height).toBe("80px");
    expect(textbox.style.overflowY).toBe("hidden");

    Object.defineProperty(textbox, "scrollHeight", { configurable: true, value: 500 });
    fireEvent.change(textbox, { target: { value: "lots\nof\nlines\n".repeat(20) } });
    // Capped at 144px and overflow becomes auto.
    expect(textbox.style.height).toBe("144px");
    expect(textbox.style.overflowY).toBe("auto");
  });

  // Axe assertions — cover empty, palette open, and chip strip rendered.
  it("passes axe in default state", async () => {
    const { container } = render(<InputBar onSend={vi.fn()} isStreaming={false} />);
    expectNoA11yViolations(await checkA11y(container));
  });

  it("passes axe with slash palette open", async () => {
    const { container } = render(<InputBar onSend={vi.fn()} isStreaming={false} />);
    fireEvent.change(screen.getByRole("textbox", { name: /chat input/i }), {
      target: { value: "/" },
    });
    expectNoA11yViolations(await checkA11y(container));
  });

  it("passes axe with chip strip rendered", async () => {
    useUIStore.setState({
      contextChips: [{ id: "chip-1", type: "account", label: "Checking" }],
    });
    const { container } = render(<InputBar onSend={vi.fn()} isStreaming={false} />);
    expectNoA11yViolations(await checkA11y(container));
  });
});
