import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SlashPalette } from "./SlashPalette";

const commands = [
  { name: "/budget", description: "Budget command" },
  { name: "/balance", description: "Balance command" },
] as const;

describe("SlashPalette", () => {
  it("renders command options and marks selected item", () => {
    render(
      <SlashPalette
        commands={commands}
        selectedIndex={1}
        onHover={vi.fn()}
        onSelect={vi.fn()}
      />,
    );

    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(2);
    expect(options[1]).toHaveAttribute("aria-selected", "true");
  });

  it("calls callbacks on hover and selection", () => {
    const onHover = vi.fn();
    const onSelect = vi.fn();

    render(
      <SlashPalette commands={commands} selectedIndex={0} onHover={onHover} onSelect={onSelect} />,
    );

    const second = screen.getAllByRole("option")[1];
    fireEvent.mouseEnter(second);
    fireEvent.mouseDown(second);

    expect(onHover).toHaveBeenCalledWith(1);
    expect(onSelect).toHaveBeenCalledWith("/balance");
  });
});
