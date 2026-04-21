import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { HealthSidebar } from "./HealthSidebar";

vi.mock("./AccountsPanel", () => ({
  AccountsPanel: () => <div>Accounts panel</div>,
}));

vi.mock("./EnvelopesPanel", () => ({
  EnvelopesPanel: () => <div>Envelopes panel</div>,
}));

vi.mock("./ComingUpPanel", () => ({
  ComingUpPanel: () => <div>Coming up panel</div>,
}));

describe("HealthSidebar", () => {
  it("shows panels when open", () => {
    render(<HealthSidebar open onToggle={vi.fn()} />);

    expect(screen.getByText(/accounts panel/i)).toBeInTheDocument();
    expect(screen.getByText(/envelopes panel/i)).toBeInTheDocument();
    expect(screen.getByText(/coming up panel/i)).toBeInTheDocument();
  });

  it("hides panels when closed", () => {
    render(<HealthSidebar open={false} onToggle={vi.fn()} />);

    expect(screen.queryByText(/accounts panel/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/envelopes panel/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/coming up panel/i)).not.toBeInTheDocument();
  });

  it("shows a visible toggle affordance", () => {
    render(<HealthSidebar open onToggle={vi.fn()} />);

    expect(screen.getByRole("button", { name: /collapse sidebar/i })).toBeInTheDocument();
  });
});
