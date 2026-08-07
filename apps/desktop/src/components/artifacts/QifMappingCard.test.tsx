import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { QifMappingCard } from "./QifMappingCard";
import type { QifImportPlan } from "@tally/core-types";

const MOCK_PLAN: QifImportPlan = {
  household_id: "hh",
  import_id: "imp",
  account_mappings: [
    {
      qif_name: "Checking",
      tally_account_id: "u1",
      tally_name: "Checking",
      tally_type: "asset",
      tally_normal_balance: "debit",
    },
    {
      qif_name: "Credit Card",
      tally_account_id: "u2",
      tally_name: "Credit Card",
      tally_type: "liability",
      tally_normal_balance: "credit",
    },
  ],
  transactions: [],
};

describe("QifMappingCard", () => {
  it("renders account names and type pills", () => {
    render(
      <QifMappingCard
        plan={MOCK_PLAN}
        skippedSecurityTrades={0}
        onConfirm={vi.fn()}
        onRequestEdit={vi.fn()}
      />,
    );
    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(screen.getByText("Credit Card")).toBeInTheDocument();
    expect(screen.getByText("asset")).toBeInTheDocument();
    expect(screen.getByText("liability")).toBeInTheDocument();
  });

  it("surfaces skipped security trade count when nonzero", () => {
    render(
      <QifMappingCard
        plan={MOCK_PLAN}
        skippedSecurityTrades={3}
        onConfirm={vi.fn()}
        onRequestEdit={vi.fn()}
      />,
    );
    expect(
      screen.getByText((_, el) =>
        el?.textContent?.replace(/\s+/g, " ").trim() === "3 security trades skipped",
      ),
    ).toBeInTheDocument();
  });

  it("hides the skipped-trade chip when zero", () => {
    render(
      <QifMappingCard
        plan={MOCK_PLAN}
        skippedSecurityTrades={0}
        onConfirm={vi.fn()}
        onRequestEdit={vi.fn()}
      />,
    );
    expect(screen.queryByText(/trades skipped/i)).not.toBeInTheDocument();
  });

  it("fires onConfirm when 'Looks right' is clicked", () => {
    const onConfirm = vi.fn();
    render(
      <QifMappingCard
        plan={MOCK_PLAN}
        skippedSecurityTrades={0}
        onConfirm={onConfirm}
        onRequestEdit={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /looks right/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("fires onRequestEdit when the change button is clicked", () => {
    const onRequestEdit = vi.fn();
    render(
      <QifMappingCard
        plan={MOCK_PLAN}
        skippedSecurityTrades={0}
        onConfirm={vi.fn()}
        onRequestEdit={onRequestEdit}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /i need to change something/i }));
    expect(onRequestEdit).toHaveBeenCalledTimes(1);
  });
});
