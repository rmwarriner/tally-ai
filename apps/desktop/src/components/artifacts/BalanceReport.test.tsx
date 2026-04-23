import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import styles from "./BalanceReport.module.css";
import { BalanceReport } from "./BalanceReport";

describe("BalanceReport", () => {
  it("renders account rows with subtotal and negative styling", () => {
    render(
      <BalanceReport
        nodes={[
          {
            account_name: "Assets",
            balance_cents: 100_000,
            depth: 0,
            is_subtotal: true,
          },
          {
            account_name: "Credit Card",
            balance_cents: -12_345,
            depth: 1,
            is_subtotal: false,
          },
        ]}
      />,
    );

    const assets = screen.getByText("Assets").closest("li");
    const creditAmount = screen.getByText("-$123.45");

    expect(assets).toHaveClass(styles.subtotal);
    expect(creditAmount).toHaveClass(styles.negative);
  });
});
