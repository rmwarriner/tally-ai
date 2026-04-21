import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { HealthSidebar } from "./HealthSidebar";

describe("HealthSidebar", () => {
  it("shows content when open", () => {
    render(<HealthSidebar open onToggle={vi.fn()} />);

    expect(screen.getByText(/health sidebar/i)).toBeInTheDocument();
  });

  it("hides content when closed", () => {
    render(<HealthSidebar open={false} onToggle={vi.fn()} />);

    expect(screen.queryByText(/health sidebar/i)).not.toBeInTheDocument();
  });

  it("shows a visible toggle affordance", () => {
    render(<HealthSidebar open onToggle={vi.fn()} />);

    expect(screen.getByRole("button", { name: /collapse sidebar/i })).toBeInTheDocument();
  });
});
