import "@testing-library/jest-dom/vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HandoffMessage, type HandoffMessageProps } from "./HandoffMessage";

const defaultProps: HandoffMessageProps = {
  householdName: "Smith Family",
  accountCount: 2,
  envelopeCount: 3,
  starterPrompts: [
    "Record my coffee this morning",
    "Show my account balances",
    "/budget",
  ],
  onPromptClick: vi.fn(),
};

describe("HandoffMessage", () => {
  it("renders household name in summary", () => {
    render(<HandoffMessage {...defaultProps} />);
    expect(screen.getByText(/Smith Family/)).toBeInTheDocument();
  });

  it("renders account count", () => {
    render(<HandoffMessage {...defaultProps} />);
    expect(screen.getByText(/2 accounts/)).toBeInTheDocument();
  });

  it("renders envelope count", () => {
    render(<HandoffMessage {...defaultProps} />);
    expect(screen.getByText(/3 envelopes/)).toBeInTheDocument();
  });

  it("renders all starter prompt buttons", () => {
    render(<HandoffMessage {...defaultProps} />);
    expect(screen.getByText("Record my coffee this morning")).toBeInTheDocument();
    expect(screen.getByText("Show my account balances")).toBeInTheDocument();
    expect(screen.getByText("/budget")).toBeInTheDocument();
  });

  it("calls onPromptClick with the prompt text when clicked", () => {
    const onPromptClick = vi.fn();
    render(<HandoffMessage {...defaultProps} onPromptClick={onPromptClick} />);
    fireEvent.click(screen.getByText("Record my coffee this morning"));
    expect(onPromptClick).toHaveBeenCalledWith("Record my coffee this morning");
  });

  it("all starter prompts are buttons (accessible)", () => {
    render(<HandoffMessage {...defaultProps} />);
    const buttons = screen.getAllByRole("button");
    expect(buttons.length).toBeGreaterThanOrEqual(3);
  });

  it("has an info circle affordance on each starter prompt", () => {
    render(<HandoffMessage {...defaultProps} />);
    const infoCircles = screen.getAllByLabelText("tap to use");
    expect(infoCircles.length).toBe(3);
  });

  it("renders a ready-to-use headline", () => {
    render(<HandoffMessage {...defaultProps} />);
    expect(screen.getByText(/ready/i)).toBeInTheDocument();
  });
});
