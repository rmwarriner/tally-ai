import axe, { type AxeResults, type RunOptions } from "axe-core";

const RULE_OVERRIDES: NonNullable<RunOptions["rules"]> = {
  // Disabled rules go here, each with a short reason that maps to
  // docs/superpowers/a11y-2026-04.md. Empty until audit is run.
};

export async function checkA11y(container: Element): Promise<AxeResults> {
  return axe.run(container, { rules: RULE_OVERRIDES });
}

export function expectNoA11yViolations(results: AxeResults): void {
  if (results.violations.length === 0) return;
  const rendered = results.violations
    .map(v => `[${v.id}] ${v.description}\n  ${v.nodes.map(n => n.html).join("\n  ")}`)
    .join("\n\n");
  throw new Error(`a11y violations:\n${rendered}`);
}
