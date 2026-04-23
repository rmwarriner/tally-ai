import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useCallback, useEffect } from "react";

import type { SetupCardVariant } from "../components/onboarding/SetupCard";
import { useChatStore } from "../stores/chatStore";
import { useOnboardingStore } from "../stores/onboardingStore";
import type { FreshStep, MigrationStep } from "../stores/onboardingStore";

export interface OnboardingDeps {
  addSystemMessage: (text: string, tone?: "info" | "error") => void;
  addSetupCard: (variant: SetupCardVariant, title: string, detail: string) => void;
  addHandoffMessage: (
    householdName: string,
    accountCount: number,
    envelopeCount: number,
    starterPrompts: string[],
  ) => void;
  invoke: typeof tauriInvoke;
}

const STARTER_PROMPTS = [
  "Record my coffee this morning",
  "Show my account balances",
  "/budget",
  "/recent",
];

function parseDollarAmount(input: string): number | null {
  const cleaned = input.replace(/[$,\s]/g, "");
  const parsed = Number.parseFloat(cleaned);
  if (Number.isNaN(parsed) || parsed < 0) return null;
  return Math.round(parsed * 100);
}

function formatCents(cents: number): string {
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(cents / 100);
}

function isAffirmative(text: string): boolean {
  return /^(yes|yeah|yep|sure|ok|okay|add|more|another|y)$/i.test(text.trim());
}

function isNegative(text: string): boolean {
  return /^(no|nope|done|none|that'?s? (all|it)|finished|stop|n)$/i.test(text.trim());
}

export function buildOnboardingHandler(deps: OnboardingDeps) {
  const store = useOnboardingStore;

  async function checkAndStart(): Promise<void> {
    const exists = await deps.invoke<boolean>("check_setup_status", {});
    if (exists) {
      store.getState().setPhase("complete");
      return;
    }
    store.getState().setPhase("path_select");
    deps.addSystemMessage(
      "Welcome to Tally! I'm your personal finance assistant. Would you like to start fresh, or import an existing hledger journal? (Say \"fresh\" or \"import\")",
      "info",
    );
  }

  async function handleFreshStep(step: FreshStep, input: string): Promise<void> {
    const state = store.getState();

    switch (step) {
      case "welcome":
        state.setFreshStep("household_name");
        deps.addSystemMessage("What would you like to call your household? (e.g. \"Smith Family\")", "info");
        return;

      case "household_name": {
        const name = input.trim();
        if (!name) {
          deps.addSystemMessage("Please enter a name for your household.", "info");
          return;
        }
        state.patchDraft({ householdName: name });
        state.setFreshStep("timezone");
        deps.addSystemMessage(
          `Got it — "${name}". What timezone are you in? (e.g. America/Chicago, America/New_York, America/Los_Angeles)`,
          "info",
        );
        return;
      }

      case "timezone": {
        const tz = input.trim();
        state.patchDraft({ timezone: tz });
        state.setFreshStep("passphrase");
        deps.addSystemMessage(
          "Now let's protect your data. Choose an encryption passphrase. Keep it safe — it cannot be recovered if lost.",
          "info",
        );
        return;
      }

      case "passphrase": {
        const passphrase = input.trim();
        state.patchDraft({ passphrase });
        state.setFreshStep("confirm_passphrase");
        deps.addSystemMessage("Please confirm your passphrase:", "info");
        return;
      }

      case "confirm_passphrase": {
        const { passphrase, householdName, timezone } = store.getState().draft;
        if (input.trim() !== passphrase) {
          store.getState().setFreshStep("passphrase");
          deps.addSystemMessage(
            "Passphrases don't match. Please choose your passphrase again:",
            "info",
          );
          return;
        }
        const id = await deps.invoke<string>("create_household", {
          name: householdName,
          timezone,
          passphrase,
        });
        store.getState().patchDraft({ passphrase: "" }); // clear from memory after use
        deps.addSetupCard(
          "household_created",
          `${householdName} household created`,
          `${timezone} · encrypted (id: ${id})`,
        );
        store.getState().setFreshStep("accounts");
        deps.addSystemMessage(
          "Your household is set up! Now let's add your bank accounts. What's your first account? (e.g. \"Chase Checking\")",
          "info",
        );
        return;
      }

      case "accounts": {
        const name = input.trim();
        store.getState().addDraftAccount({ name, type: "asset", balanceCents: 0 });
        store.getState().setFreshStep("account_balance");
        deps.addSystemMessage(`What's the current balance for "${name}"? (e.g. $1,500.00)`, "info");
        return;
      }

      case "account_balance": {
        const amountCents = parseDollarAmount(input);
        if (amountCents === null) {
          deps.addSystemMessage(
            "I couldn't read that balance. Please enter a dollar amount, like $1,500.00 or 1500.",
            "info",
          );
          return;
        }
        const currentState = store.getState();
        const idx = currentState.currentAccountIndex;
        const account = currentState.draft.accounts[idx];
        if (!account) return;

        const updatedAccounts = [...currentState.draft.accounts];
        updatedAccounts[idx] = { ...account, balanceCents: amountCents };
        currentState.patchDraft({ accounts: updatedAccounts });

        const accountId = await deps.invoke<string>("create_account", {
          name: account.name,
          account_type: account.type,
        });
        await deps.invoke("set_opening_balance", {
          account_id: accountId,
          amount_cents: amountCents,
        });

        deps.addSetupCard(
          "account_created",
          `${account.name} created`,
          `Asset · ${formatCents(amountCents)} opening balance`,
        );
        deps.addSetupCard(
          "opening_balance",
          "Opening balance set",
          `${account.name} · ${formatCents(amountCents)}`,
        );

        currentState.advanceAccountIndex();
        store.getState().setFreshStep("more_accounts");
        deps.addSystemMessage("Do you have another account to add? (yes / no)", "info");
        return;
      }

      case "more_accounts": {
        if (isAffirmative(input)) {
          store.getState().setFreshStep("accounts");
          deps.addSystemMessage("What's the next account?", "info");
        } else if (isNegative(input)) {
          store.getState().setFreshStep("envelopes");
          deps.addSystemMessage(
            "Great! Now let's create budget envelopes — categories you spend money in. What's your first one? (e.g. \"Groceries\")",
            "info",
          );
        } else {
          deps.addSystemMessage(
            'Say "yes" to add another account, or "no" to move on.',
            "info",
          );
        }
        return;
      }

      case "envelopes": {
        const name = input.trim();
        store.getState().addDraftEnvelope({ name });
        await deps.invoke("create_envelope", { name });
        deps.addSetupCard("envelope_created", `${name} envelope created`, "Budget category added");
        store.getState().setFreshStep("more_envelopes");
        deps.addSystemMessage("Add another envelope? (yes / no)", "info");
        return;
      }

      case "more_envelopes": {
        if (isAffirmative(input)) {
          store.getState().setFreshStep("envelopes");
          deps.addSystemMessage("What's the next envelope?", "info");
        } else {
          const { householdName, accounts, envelopes } = store.getState().draft;
          deps.addHandoffMessage(householdName, accounts.length, envelopes.length, STARTER_PROMPTS);
          store.getState().setPhase("complete");
        }
        return;
      }

      case "done":
        return;
    }
  }

  async function handleMigrationStep(step: MigrationStep, input: string): Promise<void> {
    const state = store.getState();

    switch (step) {
      case "welcome":
        state.setMigrationStep("file_drop");
        deps.addSystemMessage(
          "Paste your hledger journal content here, or type the path to your .journal file. I'll import it and map your accounts.",
          "info",
        );
        return;

      case "file_drop": {
        const content = input.trim();
        state.patchDraft({ hledgerContent: content });
        const summary = await deps.invoke<string>("import_hledger", { content });
        deps.addSystemMessage(`Import complete: ${summary}`, "info");
        state.setMigrationStep("coa_mapping");
        deps.addSystemMessage(
          'Your accounts have been mapped. Say "done" to finish setup.',
          "info",
        );
        return;
      }

      case "coa_mapping": {
        const { householdName } = store.getState().draft;
        deps.addHandoffMessage(householdName || "Your household", 0, 0, STARTER_PROMPTS);
        state.setPhase("complete");
        return;
      }

      case "done":
        return;
    }
  }

  async function handleInput(text: string): Promise<void> {
    const { phase, freshStep, migrationStep } = store.getState();

    switch (phase) {
      case "path_select": {
        const lower = text.toLowerCase();
        if (lower.includes("fresh") || lower.includes("start")) {
          store.getState().setPhase("fresh_start");
          deps.addSystemMessage(
            "Let's start fresh! What would you like to call your household? (e.g. \"Smith Family\")",
            "info",
          );
          store.getState().setFreshStep("household_name");
        } else if (lower.includes("import") || lower.includes("hledger") || lower.includes("journal") || lower.includes("migrate")) {
          store.getState().setPhase("migration");
          await handleMigrationStep("welcome", text);
        } else {
          deps.addSystemMessage(
            'Say "fresh" to start from scratch, or "import" to bring in an existing hledger journal.',
            "info",
          );
        }
        return;
      }

      case "fresh_start":
        await handleFreshStep(freshStep, text);
        return;

      case "migration":
        await handleMigrationStep(migrationStep, text);
        return;

      case "checking":
      case "complete":
        return;
    }
  }

  return { checkAndStart, handleInput };
}

export function useOnboardingEngine() {
  const addSystemMessage = useChatStore((s) => s.addSystemMessage);
  const addSetupCard = useChatStore((s) => s.addSetupCard);
  const addHandoffMessage = useChatStore((s) => s.addHandoffMessage);
  const phase = useOnboardingStore((s) => s.phase);

  const deps: OnboardingDeps = {
    addSystemMessage,
    addSetupCard,
    addHandoffMessage,
    invoke: tauriInvoke,
  };

  const handler = buildOnboardingHandler(deps);

  useEffect(() => {
    void handler.checkAndStart();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleInput = useCallback(
    (text: string) => handler.handleInput(text),
    // deps change reference each render but behavior is stable — rebuild handler on each call is fine
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  return {
    isActive: phase !== "complete",
    handleInput,
  };
}
