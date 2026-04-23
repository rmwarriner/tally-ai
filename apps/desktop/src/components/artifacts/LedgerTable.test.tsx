import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { LedgerTable } from "./LedgerTable";

describe("LedgerTable", () => {
  it("renders rows with formatted dates and amounts", () => {
    render(
      <LedgerTable
        rows={[
          {
            date: new Date("2026-04-01T00:00:00.000Z").getTime(),
            payee: "Whole Foods",
            amount_cents: 5432,
            side: "debit",
          },
        ]}
      />,
    );

    expect(screen.getByText("Whole Foods")).toBeInTheDocument();
    expect(screen.getByText("$54.32")).toBeInTheDocument();
  });
});
