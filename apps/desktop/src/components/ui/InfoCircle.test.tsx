import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import tooltipStyles from "./Tooltip.module.css";
import { InfoCircle } from "./InfoCircle";

describe("InfoCircle", () => {
  it("renders a non-interactive icon when onClick is absent", () => {
    render(<InfoCircle />);

    const icon = screen.getByRole("img", { name: /more information/i });
    expect(icon).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /more information/i })).not.toBeInTheDocument();
    expect(icon).not.toHaveAttribute("aria-describedby");
  });

  it("renders as a button when onClick is provided", () => {
    const onClick = vi.fn();

    render(<InfoCircle tooltip="Click me" onClick={onClick} />);

    const button = screen.getByRole("button", { name: /more information/i });
    fireEvent.click(button);

    expect(button).toBeInTheDocument();
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("uses a custom aria-label override", () => {
    render(<InfoCircle onClick={vi.fn()} aria-label="Collapse sidebar" tooltip="Toggle" />);

    expect(screen.getByRole("button", { name: /collapse sidebar/i })).toBeInTheDocument();
  });

  it("renders tooltip text and links it with aria-describedby", () => {
    render(<InfoCircle onClick={vi.fn()} tooltip="Balances are updated after posting." />);

    const button = screen.getByRole("button", { name: /more information/i });
    const tooltip = screen.getByRole("tooltip", {
      name: /balances are updated after posting\./i,
    });

    expect(tooltip).toBeInTheDocument();
    expect(button).toHaveAttribute("aria-describedby", tooltip.getAttribute("id"));
  });

  it("shows and hides the tooltip class on hover", () => {
    render(<InfoCircle onClick={vi.fn()} tooltip="Hover text" />);

    const button = screen.getByRole("button", { name: /more information/i });
    const wrapper = button.parentElement as HTMLElement;
    const tooltip = screen.getByRole("tooltip", { name: /hover text/i });

    expect(tooltip).not.toHaveClass(tooltipStyles.visible);

    fireEvent.mouseEnter(wrapper);
    expect(tooltip).toHaveClass(tooltipStyles.visible);

    fireEvent.mouseLeave(wrapper);
    expect(tooltip).not.toHaveClass(tooltipStyles.visible);
  });

  it("shows and hides the tooltip class on focus", () => {
    render(<InfoCircle onClick={vi.fn()} tooltip="Focus text" />);

    const button = screen.getByRole("button", { name: /more information/i });
    const tooltip = screen.getByRole("tooltip", { name: /focus text/i });

    fireEvent.focus(button);
    expect(tooltip).toHaveClass(tooltipStyles.visible);

    fireEvent.blur(button);
    expect(tooltip).not.toHaveClass(tooltipStyles.visible);
  });
});
