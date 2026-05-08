// E2E: /undo slash command.
//
// Validates the wrapper wiring landed in this PR (commands::undo_last_transaction
// no longer a stub). Two paths:
//   - Success: command resolves with a reversal ULID, success system message
//     appears, and the sidebar gets invalidated.
//   - No-history: command rejects with a Discard-style RecoveryError; the
//     dispatcher swallows the error and shows the standard fallback message.

import { existingHouseholdResponses, rejectWith } from "./fixtures/responses";
import { expect, test } from "./setup";

test.describe("/undo command", () => {
  test("happy path: success message renders and sidebar query is invalidated", async ({
    setupMock,
    page,
    getInvokeCalls,
  }) => {
    await setupMock({
      ...existingHouseholdResponses,
      undo_last_transaction: "01TXNREVERSAL0000000000000",
    });
    await page.goto("/");
    await page.getByLabel("Chat input").waitFor();

    const callsBefore = await getInvokeCalls();
    const sidebarFetchesBefore = callsBefore.filter((c) =>
      c.cmd.startsWith("get_account_balances"),
    ).length;

    await page.getByLabel("Chat input").fill("/undo");
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(page.getByText(/Last transaction undone/i).last()).toBeVisible();

    const calls = await getInvokeCalls();
    expect(calls.map((c) => c.cmd)).toContain("undo_last_transaction");

    // Sidebar invalidate triggers a re-fetch of get_account_balances.
    const sidebarFetchesAfter = calls.filter((c) =>
      c.cmd.startsWith("get_account_balances"),
    ).length;
    expect(sidebarFetchesAfter).toBeGreaterThan(sidebarFetchesBefore);
  });

  test("no-history path: shows the standard error system message", async ({
    setupMock,
    page,
  }) => {
    await setupMock({
      ...existingHouseholdResponses,
      undo_last_transaction: rejectWith(
        "Nothing to undo. No posted transactions to reverse.",
        "DISCARD",
      ),
    });
    await page.goto("/");
    await page.getByLabel("Chat input").waitFor();

    await page.getByLabel("Chat input").fill("/undo");
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(
      page.getByText(/Nothing to undo, or the last transaction cannot be reversed/i).last(),
    ).toBeVisible();
  });
});
