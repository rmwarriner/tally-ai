import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { GnuCashReconcileCard } from "./GnuCashReconcileCard";

const sampleReport = {
  total_mismatches: 0,
  rows: [
    { account_name: "Checking", tally_cents: 95000, gnucash_cents: 95000, matches: true },
    { account_name: "Groceries", tally_cents: 5000, gnucash_cents: 5000, matches: true },
  ],
};

const mismatchReport = {
  total_mismatches: 1,
  rows: [
    { account_name: "Checking", tally_cents: 94900, gnucash_cents: 95000, matches: false },
  ],
};

describe("GnuCashReconcileCard", () => {
  it("renders every row with tally + gnucash balance", () => {
    render(<GnuCashReconcileCard report={sampleReport} onAccept={() => {}} onRollback={() => {}} />);
    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(screen.getAllByText("$950.00").length).toBeGreaterThan(0);
  });

  it("flags mismatches visibly", () => {
    render(<GnuCashReconcileCard report={mismatchReport} onAccept={() => {}} onRollback={() => {}} />);
    expect(screen.getByText(/1 mismatch/i)).toBeInTheDocument();
  });

  it("fires onAccept when user clicks 'Looks right'", () => {
    const onAccept = vi.fn();
    render(<GnuCashReconcileCard report={sampleReport} onAccept={onAccept} onRollback={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /looks right/i }));
    expect(onAccept).toHaveBeenCalled();
  });

  it("fires onRollback when user clicks 'Roll back'", () => {
    const onRollback = vi.fn();
    render(<GnuCashReconcileCard report={sampleReport} onAccept={() => {}} onRollback={onRollback} />);
    fireEvent.click(screen.getByRole("button", { name: /roll back/i }));
    expect(onRollback).toHaveBeenCalled();
  });

  it("shows 'All balances match' headline when zero mismatches", () => {
    render(<GnuCashReconcileCard report={sampleReport} onAccept={() => {}} onRollback={() => {}} />);
    expect(screen.getByText(/all balances match/i)).toBeInTheDocument();
  });

  it("shows plural 'mismatches' for more than one mismatch", () => {
    const multiMismatch = {
      total_mismatches: 2,
      rows: [
        { account_name: "Checking", tally_cents: 94900, gnucash_cents: 95000, matches: false },
        { account_name: "Savings", tally_cents: 100, gnucash_cents: 200, matches: false },
      ],
    };
    render(<GnuCashReconcileCard report={multiMismatch} onAccept={() => {}} onRollback={() => {}} />);
    expect(screen.getByText(/2 mismatches/i)).toBeInTheDocument();
  });
});
