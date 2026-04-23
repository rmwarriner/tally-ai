import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProactiveMessage } from "./ProactiveMessage";

describe("ProactiveMessage", () => {
  it("renders with proactive avatar and note role", () => {
    render(
      <ProactiveMessage
        id="1"
        text="You may be running low on Groceries budget."
        ts={Date.now()}
      />,
    );

    expect(screen.getByRole("note", { name: /proactive advisory/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/proactive avatar/i)).toBeInTheDocument();
  });

  it("shows advisory code pill when provided", () => {
    render(
      <ProactiveMessage
        id="1"
        text="Advisory."
        ts={Date.now()}
        advisory_code="LOW_BALANCE_WARNING"
      />,
    );

    expect(screen.getByText("LOW_BALANCE_WARNING")).toBeInTheDocument();
  });

  it("does not show code pill when advisory_code is omitted", () => {
    render(<ProactiveMessage id="1" text="Advisory." ts={Date.now()} />);

    expect(screen.queryByText(/WARNING/)).not.toBeInTheDocument();
  });
});
