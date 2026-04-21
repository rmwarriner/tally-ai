import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { HealthSidebar } from "./HealthSidebar";
import { useEnvelopeStatuses, usePendingTransactions } from "../../hooks/useSidebarData";

vi.mock("../../hooks/useSidebarData", () => ({
  useEnvelopeStatuses: vi.fn(),
  usePendingTransactions: vi.fn(),
}));

vi.mock("./AccountsPanel", () => ({
  AccountsPanel: () => <div>Accounts panel</div>,
}));

vi.mock("./EnvelopesPanel", () => ({
  EnvelopesPanel: () => <div>Envelopes panel</div>,
}));

vi.mock("./ComingUpPanel", () => ({
  ComingUpPanel: () => <div>Coming up panel</div>,
}));

vi.mock("./SidebarIconStrip", () => ({
  SidebarIconStrip: () => <div>Icon strip</div>,
}));

const mockUseEnvelopeStatuses = vi.mocked(useEnvelopeStatuses);
const mockUsePendingTransactions = vi.mocked(usePendingTransactions);

describe("HealthSidebar", () => {
  beforeEach(() => {
    mockUseEnvelopeStatuses.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useEnvelopeStatuses>);

    mockUsePendingTransactions.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as ReturnType<typeof usePendingTransactions>);
  });

  it("shows panels when state is open", () => {
    render(<HealthSidebar state="open" onToggle={vi.fn()} />);

    expect(screen.getByText(/accounts panel/i)).toBeInTheDocument();
    expect(screen.getByText(/envelopes panel/i)).toBeInTheDocument();
    expect(screen.getByText(/coming up panel/i)).toBeInTheDocument();
  });

  it("shows icon strip when state is icon", () => {
    render(<HealthSidebar state="icon" onToggle={vi.fn()} />);

    expect(screen.getByText(/icon strip/i)).toBeInTheDocument();
    expect(screen.queryByText(/accounts panel/i)).not.toBeInTheDocument();
  });

  it("hides content when state is hidden", () => {
    render(<HealthSidebar state="hidden" onToggle={vi.fn()} />);

    expect(screen.queryByText(/icon strip/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/accounts panel/i)).not.toBeInTheDocument();
  });
});
