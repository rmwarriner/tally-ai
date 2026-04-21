import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { EnvelopesPanel } from "./EnvelopesPanel";
import { useEnvelopeStatuses } from "../../hooks/useSidebarData";

vi.mock("../../hooks/useSidebarData", () => ({
  useEnvelopeStatuses: vi.fn(),
}));

const mockUseEnvelopeStatuses = vi.mocked(useEnvelopeStatuses);

describe("EnvelopesPanel", () => {
  beforeEach(() => {
    mockUseEnvelopeStatuses.mockReset();
  });

  it("renders progress bar with correct aria label", () => {
    mockUseEnvelopeStatuses.mockReturnValue({
      data: [
        {
          envelope_id: "1",
          name: "Groceries",
          allocated_cents: 50000,
          spent_cents: 30000,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useEnvelopeStatuses>);

    render(<EnvelopesPanel />);

    expect(screen.getByRole("progressbar", { name: /groceries 60% used/i })).toBeInTheDocument();
  });

  it("shows over-budget label", () => {
    mockUseEnvelopeStatuses.mockReturnValue({
      data: [
        {
          envelope_id: "2",
          name: "Dining",
          allocated_cents: 20000,
          spent_cents: 25000,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useEnvelopeStatuses>);

    render(<EnvelopesPanel />);

    expect(screen.getByText(/\$50\.00 over/i)).toBeInTheDocument();
  });

  it("shows empty state when no envelopes", () => {
    mockUseEnvelopeStatuses.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useEnvelopeStatuses>);

    render(<EnvelopesPanel />);

    expect(screen.getByText(/no envelopes this month/i)).toBeInTheDocument();
  });
});
