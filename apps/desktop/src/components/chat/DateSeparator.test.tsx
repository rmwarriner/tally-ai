import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DateSeparator } from "./DateSeparator";

describe("DateSeparator", () => {
  it("renders separator role and label", () => {
    render(<DateSeparator label="Today" />);

    expect(screen.getByRole("separator", { name: "Today" })).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
  });
});
