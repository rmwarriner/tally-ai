import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AccountsPanel } from "./AccountsPanel";
import styles from "./AccountsPanel.module.css";
import { useAccountBalances } from "../../hooks/useSidebarData";

vi.mock("../../hooks/useSidebarData", () => ({
  useAccountBalances: vi.fn(),
}));

const mockUseAccountBalances = vi.mocked(useAccountBalances);

describe("AccountsPanel", () => {
  beforeEach(() => {
    mockUseAccountBalances.mockReset();
  });

  it("renders account name and formatted balance", () => {
    mockUseAccountBalances.mockReturnValue({
      data: [
        {
          id: "1",
          name: "Checking",
          type: "asset",
          balance_cents: 150050,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useAccountBalances>);

    render(<AccountsPanel />);

    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(screen.getByText("$1,500.50")).toBeInTheDocument();
  });

  it("applies caution styling for positive liability balances", () => {
    mockUseAccountBalances.mockReturnValue({
      data: [
        {
          id: "2",
          name: "Credit Card",
          type: "liability",
          balance_cents: 5000,
        },
      ],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useAccountBalances>);

    render(<AccountsPanel />);

    expect(screen.getByText("$50.00")).toHaveClass(styles.liability);
  });

  it("shows loading state", () => {
    mockUseAccountBalances.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    } as ReturnType<typeof useAccountBalances>);

    render(<AccountsPanel />);

    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("shows empty state", () => {
    mockUseAccountBalances.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useAccountBalances>);

    render(<AccountsPanel />);

    expect(screen.getByText(/no accounts yet/i)).toBeInTheDocument();
  });
});
