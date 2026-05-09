// E2E: /fix slash command.
//
// /fix routes through `useSlashDispatch` → `sendMessage("Fix: …")` which
// re-enters `submit_message`. The palette filtering, command dispatch, and
// the reformulated outbound message are what this spec validates.

import { existingHouseholdResponses, textResponse } from "./fixtures/responses";
import { expect, test } from "./setup";

test.describe("/fix command", () => {
  test.beforeEach(async ({ setupMock, page }) => {
    await setupMock({
      ...existingHouseholdResponses,
      submit_message: textResponse("Looking into that correction now."),
    });
    await page.goto("/");
    await page.getByLabel("Chat input").waitFor();
  });

  test("palette opens on / and narrows to /fix when typing /f", async ({ page }) => {
    const input = page.getByLabel("Chat input");
    await input.fill("/");
    await expect(page.getByRole("listbox", { name: "Slash commands" })).toBeVisible();
    // All commands are visible at first (count tracks SLASH_COMMANDS).
    await expect(page.getByRole("option")).toHaveCount(8);

    await input.fill("/f");
    // /fix is the only match starting with "f".
    await expect(page.getByRole("option")).toHaveCount(1);
    await expect(page.getByRole("option")).toContainText("/fix");
  });

  test("/fix dispatches a 'Fix: …' message through submit_message", async ({
    page,
    getInvokeCalls,
  }) => {
    const input = page.getByLabel("Chat input");
    await input.fill('/fix groceries on Tuesday was $45');
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(page.getByText(/Looking into that correction/i).last()).toBeVisible();

    const calls = await getInvokeCalls();
    const submit = calls.find((c) => c.cmd === "submit_message");
    expect(submit).toBeDefined();
    const args = submit!.args as { args: { text: string } };
    expect(args.args.text).toBe("Fix: groceries on Tuesday was $45");
  });
});
