import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { QifReconcileCard } from "./QifReconcileCard";

describe("QifReconcileCard", () => {
  it("renders the matching headline when there are no mismatches", () => {
    render(
      <QifReconcileCard
        report={{
          rows: [
            {
              account_name: "Checking",
              tally_cents: 12345,
              declared_cents: 12345,
              matches: true,
            },
          ],
          total_mismatches: 0,
        }}
        onAccept={vi.fn()}
        onRollback={vi.fn()}
      />,
    );
    expect(screen.getByText(/all balances match the qif file/i)).toBeInTheDocument();
    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(screen.getAllByText("$123.45")).toHaveLength(2);
  });

  it("renders a mismatch count and marker", () => {
    render(
      <QifReconcileCard
        report={{
          rows: [
            {
              account_name: "Savings",
              tally_cents: 100,
              declared_cents: 200,
              matches: false,
            },
          ],
          total_mismatches: 1,
        }}
        onAccept={vi.fn()}
        onRollback={vi.fn()}
      />,
    );
    expect(screen.getByText(/1 mismatch/i)).toBeInTheDocument();
    expect(screen.getByText("!")).toBeInTheDocument();
  });

  it("fires onAccept when continue is clicked", () => {
    const onAccept = vi.fn();
    render(
      <QifReconcileCard
        report={{ rows: [], total_mismatches: 0 }}
        onAccept={onAccept}
        onRollback={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /looks right, continue/i }));
    expect(onAccept).toHaveBeenCalledTimes(1);
  });

  it("fires onRollback when roll back is clicked", () => {
    const onRollback = vi.fn();
    render(
      <QifReconcileCard
        report={{ rows: [], total_mismatches: 0 }}
        onAccept={vi.fn()}
        onRollback={onRollback}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /roll back/i }));
    expect(onRollback).toHaveBeenCalledTimes(1);
  });

  it("formats negative balances with a minus sign", () => {
    render(
      <QifReconcileCard
        report={{
          rows: [
            {
              account_name: "Card",
              tally_cents: -5000,
              declared_cents: -5000,
              matches: true,
            },
          ],
          total_mismatches: 0,
        }}
        onAccept={vi.fn()}
        onRollback={vi.fn()}
      />,
    );
    expect(screen.getAllByText("-$50.00")).toHaveLength(2);
  });
});
