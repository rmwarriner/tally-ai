import { describe, expect, it } from "vitest";

import { formatCents } from "./formatCents";

describe("formatCents", () => {
  it("formats positive cents as dollars", () => {
    expect(formatCents(1234)).toBe("$12.34");
  });

  it("formats zero", () => {
    expect(formatCents(0)).toBe("$0.00");
  });

  it("formats negative cents", () => {
    expect(formatCents(-500)).toBe("-$5.00");
  });
});
