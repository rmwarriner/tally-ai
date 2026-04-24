import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { GnuCashMappingCard } from "./GnuCashMappingCard";
import type { ImportPlan } from "@tally/core-types";

const MOCK_PLAN: ImportPlan = {
  household_id: "hh",
  import_id: "imp",
  account_mappings: [
    {
      gnc_guid: "a1",
      gnc_full_name: "Assets:Checking",
      tally_account_id: "u1",
      tally_name: "Checking",
      tally_parent_id: null,
      tally_type: "asset",
      tally_normal_balance: "debit",
    },
    {
      gnc_guid: "a2",
      gnc_full_name: "Liabilities:Credit Card",
      tally_account_id: "u2",
      tally_name: "Credit Card",
      tally_parent_id: null,
      tally_type: "liability",
      tally_normal_balance: "credit",
    },
    {
      gnc_guid: "a3",
      gnc_full_name: "Expenses:Groceries",
      tally_account_id: "u3",
      tally_name: "Groceries",
      tally_parent_id: null,
      tally_type: "expense",
      tally_normal_balance: "debit",
    },
  ],
  transactions: [],
};

describe("GnuCashMappingCard", () => {
  it("renders account count and transaction count in the header", () => {
    render(
      <GnuCashMappingCard
        plan={MOCK_PLAN}
        onConfirm={vi.fn()}
        onRequestEdit={vi.fn()}
      />,
    );
    // Text is split across elements: <strong>3</strong> accounts
    // Use a function matcher to check container text
    expect(
      screen.getByText((_, element) =>
        element?.textContent?.replace(/\s+/g, " ").trim() === "3 accounts",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText((_, element) =>
        element?.textContent?.replace(/\s+/g, " ").trim() === "0 transactions",
      ),
    ).toBeInTheDocument();
  });

  it("renders every account full name", () => {
    render(
      <GnuCashMappingCard
        plan={MOCK_PLAN}
        onConfirm={vi.fn()}
        onRequestEdit={vi.fn()}
      />,
    );
    expect(screen.getByText("Assets:Checking")).toBeInTheDocument();
    expect(screen.getByText("Liabilities:Credit Card")).toBeInTheDocument();
    expect(screen.getByText("Expenses:Groceries")).toBeInTheDocument();
  });

  it("renders account type pills for each account", () => {
    render(
      <GnuCashMappingCard
        plan={MOCK_PLAN}
        onConfirm={vi.fn()}
        onRequestEdit={vi.fn()}
      />,
    );
    // Each account type appears as a pill
    expect(screen.getByText("asset")).toBeInTheDocument();
    expect(screen.getByText("liability")).toBeInTheDocument();
    expect(screen.getByText("expense")).toBeInTheDocument();
  });

  it("fires onConfirm when 'Looks right' button is clicked", () => {
    const onConfirm = vi.fn();
    render(
      <GnuCashMappingCard
        plan={MOCK_PLAN}
        onConfirm={onConfirm}
        onRequestEdit={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /looks right/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("fires onRequestEdit when 'I need to change something' button is clicked", () => {
    const onRequestEdit = vi.fn();
    render(
      <GnuCashMappingCard
        plan={MOCK_PLAN}
        onConfirm={vi.fn()}
        onRequestEdit={onRequestEdit}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /i need to change something/i }));
    expect(onRequestEdit).toHaveBeenCalledTimes(1);
  });
});
