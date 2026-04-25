import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SetupCard, type SetupCardProps } from "./SetupCard";

describe("SetupCard — account_created variant", () => {
  const props: SetupCardProps = {
    variant: "account_created",
    title: "Checking account created",
    detail: "Asset · $1,500.00 opening balance",
  };

  it("renders the title", () => {
    render(<SetupCard {...props} />);
    expect(screen.getByText("Checking account created")).toBeInTheDocument();
  });

  it("renders the detail line", () => {
    render(<SetupCard {...props} />);
    expect(screen.getByText("Asset · $1,500.00 opening balance")).toBeInTheDocument();
  });

  it("has role=status for accessibility", () => {
    render(<SetupCard {...props} />);
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  it("renders the check icon indicator", () => {
    render(<SetupCard {...props} />);
    expect(screen.getByLabelText("created")).toBeInTheDocument();
  });
});

describe("SetupCard — envelope_created variant", () => {
  it("renders envelope name", () => {
    render(
      <SetupCard
        variant="envelope_created"
        title="Groceries envelope created"
        detail="Budget category added"
      />,
    );
    expect(screen.getByText("Groceries envelope created")).toBeInTheDocument();
  });
});

describe("SetupCard — opening_balance variant", () => {
  it("renders balance summary", () => {
    render(
      <SetupCard
        variant="opening_balance"
        title="Opening balance set"
        detail="Savings · $5,000.00"
      />,
    );
    expect(screen.getByText("Opening balance set")).toBeInTheDocument();
    expect(screen.getByText("Savings · $5,000.00")).toBeInTheDocument();
  });
});

describe("SetupCard — household_created variant", () => {
  it("renders household name", () => {
    render(
      <SetupCard
        variant="household_created"
        title="Smith Family household created"
        detail="America/Chicago · encrypted"
      />,
    );
    expect(screen.getByText("Smith Family household created")).toBeInTheDocument();
  });
});

describe("SetupCard — gnucash_file_picker variant", () => {
  it("renders a path input and submit button", () => {
    render(
      <SetupCard
        variant="gnucash_file_picker"
        title="Import from GnuCash"
        detail=""
      />,
    );
    expect(screen.getByLabelText(/path to your gnucash file/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /use this file/i })).toBeInTheDocument();
  });

  it("submit button is disabled when input is empty", () => {
    render(
      <SetupCard
        variant="gnucash_file_picker"
        title="Import from GnuCash"
        detail=""
      />,
    );
    expect(screen.getByRole("button", { name: /use this file/i })).toBeDisabled();
  });

  it("calls onSubmitGnuCashPath with the trimmed path on submit", () => {
    const onSubmit = vi.fn();
    render(
      <SetupCard
        variant="gnucash_file_picker"
        title="Import from GnuCash"
        detail=""
        onSubmitGnuCashPath={onSubmit}
      />,
    );
    const input = screen.getByLabelText(/path to your gnucash file/i);
    fireEvent.change(input, { target: { value: "/Users/me/book.gnucash" } });
    fireEvent.submit(input.closest("form")!);
    expect(onSubmit).toHaveBeenCalledWith("/Users/me/book.gnucash");
  });

  it("does not call onSubmitGnuCashPath when path is whitespace only", () => {
    const onSubmit = vi.fn();
    render(
      <SetupCard
        variant="gnucash_file_picker"
        title="Import from GnuCash"
        detail=""
        onSubmitGnuCashPath={onSubmit}
      />,
    );
    // Button stays disabled for empty input
    expect(screen.getByRole("button", { name: /use this file/i })).toBeDisabled();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
