import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
