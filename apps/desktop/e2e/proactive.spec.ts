// E2E: proactive engine (T-050–T-054).
//
// Covers three integration points:
//   1. session_open returns a morning briefing → it lands as a proactive
//      message with the briefing accessible name.
//   2. commit_proposal returns proactive_insights[] (envelope-over alert) →
//      each insight renders as a proactive message.
//   3. /sensitivity quiet|normal|proactive dispatches set_sensitivity and
//      surfaces the resulting status.

import {
  committedResponse,
  envelopeOverInsight,
  existingHouseholdResponses,
  morningBriefingResponse,
  proposalResponse,
} from "./fixtures/responses";
import { expect, test } from "./setup";

test.describe("proactive engine", () => {
  test("morning briefing renders on session open with briefing aria-label", async ({
    setupMock,
    page,
  }) => {
    await setupMock({
      ...existingHouseholdResponses,
      session_open: morningBriefingResponse(),
    });
    await page.goto("/");

    await expect(page.getByRole("note", { name: /morning briefing/i })).toBeVisible();
    await expect(page.getByText(/Good morning/i).last()).toBeVisible();
  });

  test("envelope-over alert renders after commit_proposal returns insights", async ({
    setupMock,
    page,
  }) => {
    await setupMock({
      ...existingHouseholdResponses,
      submit_message: proposalResponse("Coffee", 500),
      commit_proposal: {
        ...committedResponse("01TXNCOMMITTED"),
        proactive_insights: [envelopeOverInsight("Groceries", 1_000)],
      },
    });
    await page.goto("/");
    await page.getByLabel("Chat input").waitFor();

    await page.getByLabel("Chat input").fill("$5 coffee");
    await page.getByRole("button", { name: "Send message" }).click();
    await page.getByRole("button", { name: "Confirm" }).click();

    await expect(page.getByRole("note", { name: /proactive alert/i })).toBeVisible();
    await expect(page.getByText(/Groceries is over budget/i).last()).toBeVisible();
  });

  test("/sensitivity quiet sets the sensitivity and confirms via system message", async ({
    setupMock,
    page,
    getInvokeCalls,
  }) => {
    await setupMock(existingHouseholdResponses);
    await page.goto("/");
    await page.getByLabel("Chat input").waitFor();

    await page.getByLabel("Chat input").fill("/sensitivity quiet");
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(page.getByText(/Sensitivity set to quiet/i).last()).toBeVisible();

    const calls = await getInvokeCalls();
    const setCall = calls.find((c) => c.cmd === "set_sensitivity");
    expect(setCall).toBeDefined();
    expect((setCall!.args as { args: { sensitivity: string } }).args.sensitivity).toBe("quiet");
  });

  test("/sensitivity with no args reads the current value", async ({
    setupMock,
    page,
    getInvokeCalls,
  }) => {
    await setupMock({
      ...existingHouseholdResponses,
      get_sensitivity: "proactive",
    });
    await page.goto("/");
    await page.getByLabel("Chat input").waitFor();

    await page.getByLabel("Chat input").fill("/sensitivity");
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(page.getByText(/Sensitivity is set to proactive/i).last()).toBeVisible();

    const calls = await getInvokeCalls();
    expect(calls.map((c) => c.cmd)).toContain("get_sensitivity");
  });
});
