import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ComingUpPanel } from "./ComingUpPanel";
import { usePendingTransactions } from "../../hooks/useSidebarData";

vi.mock("../../hooks/useSidebarData", () => ({
  usePendingTransactions: vi.fn(),
}));

const mockUsePendingTransactions = vi.mocked(usePendingTransactions);

describe("ComingUpPanel", () => {
  beforeEach(() => {
    mockUsePendingTransactions.mockReset();
  });

  it("renders pending transactions", () => {
    mockUsePendingTransactions.mockReturnValue({
      data: [
        {
          id: "1",
          txn_date: new Date("2026-04-22T00:00:00.000Z").getTime(),
          payee: "Internet Bill",
          amount_cents: 8999,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof usePendingTransactions>);

    render(<ComingUpPanel />);

    expect(screen.getByText("Internet Bill")).toBeInTheDocument();
    expect(screen.getByText("$89.99")).toBeInTheDocument();
  });

  it("uses memo when payee is absent", () => {
    mockUsePendingTransactions.mockReturnValue({
      data: [
        {
          id: "2",
          txn_date: new Date("2026-04-23T00:00:00.000Z").getTime(),
          memo: "Power bill",
          amount_cents: 12000,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof usePendingTransactions>);

    render(<ComingUpPanel />);

    expect(screen.getByText("Power bill")).toBeInTheDocument();
  });

  it("falls back to untitled when payee and memo are absent", () => {
    mockUsePendingTransactions.mockReturnValue({
      data: [
        {
          id: "3",
          txn_date: new Date("2026-04-24T00:00:00.000Z").getTime(),
          amount_cents: 100,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof usePendingTransactions>);

    render(<ComingUpPanel />);

    expect(screen.getByText(/untitled transaction/i)).toBeInTheDocument();
  });

  it("shows empty state", () => {
    mockUsePendingTransactions.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as ReturnType<typeof usePendingTransactions>);

    render(<ComingUpPanel />);

    expect(screen.getByText(/no pending transactions/i)).toBeInTheDocument();
  });
});
