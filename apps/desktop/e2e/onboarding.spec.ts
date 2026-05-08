// E2E: fresh-start onboarding flow.
//
// Mock-invoke harness drives the app against canned responses; this spec
// validates that the React state machine in `useOnboardingEngine` walks the
// user through household → tz → passphrase → accounts → envelopes → api_key
// → handoff. Migration / GnuCash paths are out of scope for T-062.
//
// `.last()` on every text matcher: messages re-occur across the flow
// (e.g. "confirm your passphrase" after a mismatch) and React.StrictMode
// double-fires effects in dev — both produce duplicates that fail strict-mode
// locator queries. The most recent match is what the user just saw.

import { freshDeviceResponses } from "./fixtures/responses";
import { expect, test } from "./setup";

async function expectLastVisible(page: import("@playwright/test").Page, pattern: RegExp) {
  await expect(page.getByText(pattern).last()).toBeVisible();
}

test.describe("fresh-start onboarding", () => {
  test.beforeEach(async ({ setupMock, page }) => {
    await setupMock(freshDeviceResponses);
    await page.goto("/");
  });

  test("walks a new user through the full happy path to handoff", async ({
    page,
    getInvokeCalls,
  }) => {
    const input = page.getByLabel("Chat input");
    const send = page.getByRole("button", { name: "Send message" });

    await expectLastVisible(page, /Welcome to Tally/i);

    await input.fill("fresh");
    await send.click();
    await expectLastVisible(page, /call your household/i);

    await input.fill("Smith Family");
    await send.click();
    await expectLastVisible(page, /timezone/i);

    await input.fill("America/Chicago");
    await send.click();
    await expectLastVisible(page, /encryption passphrase/i);

    await input.fill("correct horse battery staple");
    await send.click();
    await expectLastVisible(page, /confirm your passphrase/i);

    await input.fill("correct horse battery staple");
    await send.click();
    await expectLastVisible(page, /Smith Family household created/i);
    await expectLastVisible(page, /first account/i);

    await input.fill("Chase Checking");
    await send.click();
    await expectLastVisible(page, /balance for "Chase Checking"/i);

    await input.fill("$1,500.00");
    await send.click();
    await expectLastVisible(page, /Chase Checking created/i);
    await expectLastVisible(page, /another account to add/i);

    await input.fill("no");
    await send.click();
    await expectLastVisible(page, /budget envelopes/i);

    await input.fill("Groceries");
    await send.click();
    await expectLastVisible(page, /Groceries envelope created/i);
    await expectLastVisible(page, /Add another envelope/i);

    await input.fill("no");
    await send.click();
    await expectLastVisible(page, /Claude API key/i);

    await input.fill("sk-ant-api03-test-key");
    await send.click();
    await expectLastVisible(page, /API key saved/i);

    const calls = await getInvokeCalls();
    const cmds = calls.map((c) => c.cmd);
    expect(cmds).toContain("create_household");
    expect(cmds).toContain("create_account");
    expect(cmds).toContain("set_opening_balance");
    expect(cmds).toContain("create_envelope");
    expect(cmds).toContain("set_api_key");
  });

  test("retries when the confirmation passphrase doesn't match", async ({ page }) => {
    const input = page.getByLabel("Chat input");
    const send = page.getByRole("button", { name: "Send message" });

    for (const value of ["fresh", "Smith Family", "America/Chicago", "first-attempt"]) {
      await input.fill(value);
      await send.click();
    }
    await expectLastVisible(page, /confirm your passphrase/i);

    await input.fill("MISMATCH");
    await send.click();
    await expectLastVisible(page, /Passphrases don't match/i);

    // Engine drops back to passphrase step; re-entering should re-prompt confirm.
    await input.fill("second-attempt");
    await send.click();
    await expectLastVisible(page, /confirm your passphrase/i);

    await input.fill("second-attempt");
    await send.click();
    await expectLastVisible(page, /Smith Family household created/i);
  });

  test("lets the user skip the API key step and still reaches handoff", async ({
    page,
    getInvokeCalls,
  }) => {
    const input = page.getByLabel("Chat input");
    const send = page.getByRole("button", { name: "Send message" });

    for (const value of [
      "fresh",
      "Smith Family",
      "America/Chicago",
      "pp",
      "pp",
      "Chase Checking",
      "$1,500.00",
      "no",
      "Groceries",
      "no",
    ]) {
      await input.fill(value);
      await send.click();
    }

    await expectLastVisible(page, /Claude API key/i);

    await input.fill("skip");
    await send.click();
    await expectLastVisible(page, /Chat features that need the AI will be unavailable/i);

    const calls = await getInvokeCalls();
    expect(calls.map((c) => c.cmd)).not.toContain("set_api_key");
  });
});
