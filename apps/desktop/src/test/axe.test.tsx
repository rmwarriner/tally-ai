import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { checkA11y, expectNoA11yViolations } from "./axe";

describe("axe helper", () => {
  it("passes on a clean button", async () => {
    const { container } = render(<button type="button">Hello</button>);
    const results = await checkA11y(container);
    expectNoA11yViolations(results);
    expect(results.violations).toEqual([]);
  });
});
