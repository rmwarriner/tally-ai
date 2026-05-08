// E2E: chat entry flow.
//
// User types → `submit_message` → text reply OR proposal card with Confirm /
// Discard. Validates the four happy-and-sad paths through `useSendMessage`
// and `useCommitProposal`.

import {
  committedResponse,
  existingHouseholdResponses,
  proposalResponse,
  rejectedValidationResponse,
  textResponse,
} from "./fixtures/responses";
import { expect, test } from "./setup";

async function bootApp(
  setupMock: (responses: Record<string, unknown>) => Promise<void>,
  page: import("@playwright/test").Page,
  extra: Record<string, unknown> = {},
) {
  await setupMock({ ...existingHouseholdResponses, ...extra });
  await page.goto("/");
  await page.getByLabel("Chat input").waitFor();
}

test.describe("chat entry", () => {
  test("renders an AI text response when submit_message returns text", async ({
    setupMock,
    page,
  }) => {
    await bootApp(setupMock, page, {
      submit_message: textResponse("Your checking balance is $1,500.00."),
    });

    await page.getByLabel("Chat input").fill("balance?");
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(
      page.getByText("Your checking balance is $1,500.00.").last(),
    ).toBeVisible();
  });

  test("renders a pending transaction card with Confirm / Discard for a proposal", async ({
    setupMock,
    page,
  }) => {
    await bootApp(setupMock, page, {
      submit_message: proposalResponse("Coffee at Blue Bottle", 500),
    });

    await page.getByLabel("Chat input").fill("$5 coffee at blue bottle");
    await page.getByRole("button", { name: "Send message" }).click();

    await expect(page.getByText("Coffee at Blue Bottle").last()).toBeVisible();
    await expect(page.getByText("Proposed").last()).toBeVisible();
    await expect(page.getByRole("button", { name: "Confirm" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Discard" })).toBeVisible();
  });

  test("Confirm posts the proposal and flips the card to posted", async ({
    setupMock,
    page,
    getInvokeCalls,
  }) => {
    await bootApp(setupMock, page, {
      submit_message: proposalResponse("Coffee", 500),
      commit_proposal: committedResponse("01TXNCOMMITTED"),
    });

    await page.getByLabel("Chat input").fill("$5 coffee");
    await page.getByRole("button", { name: "Send message" }).click();
    await page.getByRole("button", { name: "Confirm" }).click();

    // Confirm/Discard go away once the card moves to posted state.
    await expect(page.getByRole("button", { name: "Confirm" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Discard" })).toHaveCount(0);

    const calls = await getInvokeCalls();
    expect(calls.map((c) => c.cmd)).toContain("commit_proposal");
  });

  test("Discard removes the pending card without committing", async ({
    setupMock,
    page,
    getInvokeCalls,
  }) => {
    await bootApp(setupMock, page, {
      submit_message: proposalResponse("Coffee", 500),
    });

    await page.getByLabel("Chat input").fill("$5 coffee");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.getByRole("button", { name: "Discard" })).toBeVisible();
    await page.getByRole("button", { name: "Discard" }).click();

    await expect(page.getByRole("button", { name: "Confirm" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Discard" })).toHaveCount(0);
    // Proposal card (role=article) is gone; the user's input message stays.
    await expect(page.getByRole("article")).toHaveCount(0);

    const calls = await getInvokeCalls();
    expect(calls.map((c) => c.cmd)).not.toContain("commit_proposal");
  });

  test("validation rejection keeps the card pending and surfaces the error", async ({
    setupMock,
    page,
  }) => {
    await bootApp(setupMock, page, {
      submit_message: proposalResponse("Coffee", 500),
      commit_proposal: rejectedValidationResponse("That account doesn't exist."),
    });

    await page.getByLabel("Chat input").fill("$5 coffee");
    await page.getByRole("button", { name: "Send message" }).click();
    await page.getByRole("button", { name: "Confirm" }).click();

    // Card stays pending — Confirm button is still in the DOM.
    await expect(page.getByRole("button", { name: "Confirm" })).toBeVisible();
    // Error surfaces both card-local (role="alert") and as a system message.
    await expect(page.getByRole("alert").last()).toContainText(
      /That account doesn't exist/,
    );
  });
});
