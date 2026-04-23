import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import styles from "./TransactionCard.module.css";
import { TransactionCard } from "./TransactionCard";
import type { TransactionDisplay } from "./TransactionCard.types";

function makeTransaction(overrides: Partial<TransactionDisplay> = {}): TransactionDisplay {
  return {
    id: "01HV",
    payee: "Trader Joe's",
    txn_date: new Date("2026-04-23T00:00:00.000Z").getTime(),
    amount_cents: 4299,
    account_name: "Checking",
    lines: [
      {
        side: "debit",
        account_name: "Groceries",
        envelope_name: "Food",
        amount_cents: 4299,
      },
      {
        side: "credit",
        account_name: "Checking",
        amount_cents: 4299,
      },
    ],
    ...overrides,
  };
}

describe("TransactionCard", () => {
  it("renders posted card with payee, amount, and article aria-label", () => {
    render(<TransactionCard state="posted" transaction={makeTransaction()} />);

    expect(screen.getByText("Trader Joe's")).toBeInTheDocument();
    expect(screen.getByText("$42.99")).toBeInTheDocument();
    expect(screen.getByRole("article", { name: /transaction: trader joe's, \$42.99/i })).toBeInTheDocument();
  });

  it("renders pending badge and sends post command", () => {
    const onSendMessage = vi.fn();
    render(
      <TransactionCard state="pending" transaction={makeTransaction()} onSendMessage={onSendMessage} />,
    );

    expect(screen.getByText(/pending/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /post now/i }));
    expect(onSendMessage).toHaveBeenCalledWith("/fix post 01HV");
  });

  it("shows Confirm and Discard buttons when proposal callbacks are provided", () => {
    const onConfirm = vi.fn();
    const onDiscard = vi.fn();
    render(
      <TransactionCard
        state="pending"
        transaction={makeTransaction()}
        onConfirm={onConfirm}
        onDiscard={onDiscard}
      />,
    );

    expect(screen.getByText(/proposed/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /post now/i })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /confirm/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /discard/i }));
    expect(onDiscard).toHaveBeenCalledTimes(1);
  });

  it("disables proposal buttons while committing and shows a saving label", () => {
    render(
      <TransactionCard
        state="pending"
        transaction={makeTransaction()}
        onConfirm={() => undefined}
        onDiscard={() => undefined}
        isCommitting
      />,
    );

    expect(screen.getByRole("button", { name: /saving/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /discard/i })).toBeDisabled();
  });

  it("renders commitError as an alert above the actions", () => {
    render(
      <TransactionCard
        state="pending"
        transaction={makeTransaction()}
        onConfirm={() => undefined}
        onDiscard={() => undefined}
        commitError="Account balance would go negative."
      />,
    );

    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Account balance would go negative.");
  });

  it("renders voided card with strikethrough payee and amount", () => {
    render(<TransactionCard state="voided" transaction={makeTransaction()} />);

    const payee = screen.getByText("Trader Joe's");
    const amount = screen.getByText("$42.99");
    expect(payee).toHaveClass(styles.struck);
    expect(amount).toHaveClass(styles.struck);
    expect(screen.getByText(/voided/i)).toBeInTheDocument();
  });

  it("renders correction pair with original and replacement cards", () => {
    render(
      <TransactionCard
        state="correction_pair"
        transaction={makeTransaction()}
        replacement={makeTransaction({
          id: "02HV",
          payee: "Trader Joes (corrected)",
          amount_cents: 4599,
        })}
      />,
    );

    expect(screen.getByRole("article", { name: /correction: trader joe's/i })).toBeInTheDocument();
    expect(screen.getByText("Trader Joe's")).toBeInTheDocument();
    expect(screen.getByText("Trader Joes (corrected)")).toBeInTheDocument();
    expect(screen.getByText("corrected ↓")).toBeInTheDocument();
  });

  it("keeps journal lines collapsed by default and toggles open", () => {
    render(<TransactionCard state="posted" transaction={makeTransaction()} />);

    expect(screen.queryByText(/groceries/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /show journal lines/i }));
    expect(screen.getByText(/groceries \/ food/i)).toBeInTheDocument();
    expect(screen.getByRole("img", { name: /more information/i })).toBeInTheDocument();
  });
});
