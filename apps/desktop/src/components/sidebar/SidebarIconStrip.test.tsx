import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SidebarIconStrip } from "./SidebarIconStrip";

describe("SidebarIconStrip", () => {
  it("shows red dot on envelopes icon when over budget", () => {
    render(
      <SidebarIconStrip
        envelopeAlert="danger"
        hasPending={false}
        onToggle={vi.fn()}
      />
    );

    expect(screen.getByLabelText(/envelopes alert/i)).toBeInTheDocument();
  });

  it("shows blue dot on coming up icon when pending transactions exist", () => {
    render(
      <SidebarIconStrip
        envelopeAlert="none"
        hasPending
        onToggle={vi.fn()}
      />
    );

    expect(screen.getByLabelText(/coming up alert/i)).toBeInTheDocument();
  });

  it("always has a visible toggle affordance", () => {
    render(
      <SidebarIconStrip
        envelopeAlert="none"
        hasPending={false}
        onToggle={vi.fn()}
      />
    );

    expect(screen.getByRole("button", { name: /hide sidebar/i })).toBeInTheDocument();
  });
});
