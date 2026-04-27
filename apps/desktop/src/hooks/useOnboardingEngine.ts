import type { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo } from "react";

import type { GnuCashPreview, ImportPlan, ImportReceipt, MappingEdit, ImportAccountType, NormalBalance } from "@tally/core-types";
import type { SetupCardVariant } from "../components/onboarding/SetupCard";
import type { GnuCashReconcileReport } from "../components/artifacts/GnuCashReconcileCard";
import { safeInvoke } from "../lib/safeInvoke";
import { useChatStore } from "../stores/chatStore";
import { useOnboardingStore } from "../stores/onboardingStore";
import type { FreshStep, MigrationStep } from "../stores/onboardingStore";
import { useInvalidateSidebar } from "./useInvalidateSidebar";

/// Routes a Tauri command through `safeInvoke` (so this hook doesn't directly
/// import `invoke`), but preserves the existing throw-on-error semantics that
/// the onboarding engine is built around.
async function invokeOrThrow<T>(
  injected: typeof tauriInvoke | undefined,
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const r = await safeInvoke<T>(cmd, args, { invoke: injected });
  if (!r.ok) throw r.error;
  return r.value;
}

export interface OnboardingDeps {
  addSystemMessage: (text: string, tone?: "info" | "error") => void;
  addSetupCard: (variant: SetupCardVariant, title: string, detail: string) => void;
  addHandoffMessage: (
    householdName: string,
    accountCount: number,
    envelopeCount: number,
    starterPrompts: string[],
  ) => void;
  addGnuCashMappingMessage: (plan: ImportPlan) => void;
  addGnuCashReconcileMessage: (report: GnuCashReconcileReport) => void;
  invoke?: typeof tauriInvoke;
  invalidateSidebar: () => void | Promise<void>;
  readGnuCashFile: (path: string) => Promise<GnuCashPreview>;
  gnucashBuildDefaultPlan: (path: string) => Promise<ImportPlan>;
  gnucashApplyMappingEdit: (edit: MappingEdit) => Promise<ImportPlan>;
  commitGnuCashImport: () => Promise<ImportReceipt>;
  reconcileGnuCashImport: (importId: string, path: string) => Promise<GnuCashReconcileReport>;
  rollbackGnuCashImport: (importId: string) => Promise<void>;
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

function isValidAccountName(name: string): boolean {
  const trimmed = name.trim();
  return trimmed.length > 2 && !/^(a|an|the)$/i.test(trimmed);
}

function parseMappingEdit(text: string): MappingEdit | null {
  const changeType = text.match(/make\s+(\S+(?:\s+\S+)*?)\s+(?:an?\s+)?(asset|liability|income|expense|equity)\b/i);
  if (changeType) {
    const [, name, type] = changeType;
    if (!isValidAccountName(name)) return null;
    const new_type = type.toLowerCase() as ImportAccountType;
    const new_normal_balance: NormalBalance =
      new_type === "asset" || new_type === "expense" ? "debit" : "credit";
    return { kind: "change_type", gnc_full_name: name.trim(), new_type, new_normal_balance };
  }
  const rename = text.match(/rename\s+(\S+(?:\s+\S+)*?)\s+to\s+(.+)/i);
  if (rename) {
    const [, name, newName] = rename;
    if (!isValidAccountName(name)) return null;
    return { kind: "rename", gnc_full_name: name.trim(), new_tally_name: newName.trim() };
  }
  return null;
}

export function buildOnboardingHandler(deps: OnboardingDeps) {
  const store = useOnboardingStore;

  async function checkAndStart(): Promise<void> {
    const exists = await invokeOrThrow<boolean>(deps.invoke, "check_setup_status", {});
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
        const id = await invokeOrThrow<string>(deps.invoke, "create_household", {
          name: householdName,
          timezone,
          passphrase,
        });
        void deps.invalidateSidebar();
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

        const accountId = await invokeOrThrow<string>(deps.invoke, "create_account", {
          name: account.name,
          account_type: account.type,
        });
        void deps.invalidateSidebar();
        await invokeOrThrow<void>(deps.invoke, "set_opening_balance", {
          account_id: accountId,
          amount_cents: amountCents,
        });
        void deps.invalidateSidebar();

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
        await invokeOrThrow<void>(deps.invoke, "create_envelope", { name });
        void deps.invalidateSidebar();
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
          store.getState().setFreshStep("api_key");
          deps.addSystemMessage(
            "Last step: paste your Claude API key so I can help you log transactions. Find it at https://console.anthropic.com/settings/keys. Say \"skip\" if you'd rather set it up later.",
            "info",
          );
        }
        return;
      }

      case "api_key": {
        const text = input.trim();
        if (/^skip$/i.test(text)) {
          deps.addSystemMessage(
            "No problem — you can add it later from settings. Chat features that need the AI will be unavailable until then.",
            "info",
          );
        } else if (text.length > 0) {
          try {
            await invokeOrThrow<void>(deps.invoke, "set_api_key", { key: text });
            deps.addSetupCard(
              "household_created",
              "API key saved",
              "Stored securely in your OS keychain",
            );
          } catch (err) {
            // `invokeOrThrow` throws the normalized RecoveryError, which
            // always has a `.message` string. Fall back to Error/String for
            // any non-RecoveryError throws callers might wire up.
            const detail =
              typeof err === "object" && err !== null && "message" in err && typeof (err as { message: unknown }).message === "string"
                ? (err as { message: string }).message
                : err instanceof Error
                  ? err.message
                  : String(err);
            deps.addSystemMessage(
              `Couldn't save that key: ${detail}. Try again or say "skip".`,
              "error",
            );
            return;
          }
        } else {
          deps.addSystemMessage('Paste your API key, or say "skip".', "info");
          return;
        }

        const { householdName, accounts, envelopes } = store.getState().draft;
        deps.addHandoffMessage(householdName, accounts.length, envelopes.length, STARTER_PROMPTS);
        store.getState().setPhase("complete");
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
        const summary = await invokeOrThrow<string>(deps.invoke, "import_hledger", { content });
        void deps.invalidateSidebar();
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
        if (/migrate.*gnucash|gnucash.*migrat|gnucash/i.test(text)) {
          store.getState().setPhase("gnucash_import_pick_file");
          deps.addSetupCard(
            "gnucash_file_picker",
            "Import from GnuCash",
            "Select your GnuCash file to get started",
          );
          deps.addSystemMessage(
            "Great! Please select your GnuCash file using the file picker above.",
            "info",
          );
          return;
        }
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

      case "gnucash_import_pick_file":
        // In this phase, users interact via the file picker component (handleFilePicked).
        // Text messages are ignored here.
        return;

      case "gnucash_import_mapping": {
        const trimmed = text.trim().toLowerCase();
        if (trimmed === "cancel") {
          store.getState().setGnuCashPickedPath(null);
          store.getState().setPhase("gnucash_import_pick_file");
          deps.addSystemMessage("Cancelled. Pick a GnuCash file to try again.", "info");
          return;
        }
        const edit = parseMappingEdit(text);
        if (edit) {
          const updatedPlan = await deps.gnucashApplyMappingEdit(edit);
          deps.addGnuCashMappingMessage(updatedPlan);
        } else {
          deps.addSystemMessage(
            "Try: 'make Groceries a liability' or 'rename Groceries to Food'",
            "info",
          );
        }
        return;
      }

      case "gnucash_import_committing":
        return;

      case "gnucash_import_reconciling": {
        const text2 = text.trim().toLowerCase();
        if (text2 === "continue" || text2 === "keep") {
          await handleAcceptReconcile();
        } else if (text2 === "rollback" || text2 === "roll back" || text2 === "cancel") {
          await handleRollbackReconcile();
        } else {
          deps.addSystemMessage(
            "Type 'continue' to keep the import, or 'rollback' to undo it.",
            "info",
          );
        }
        return;
      }

      case "gnucash_import_done":
      case "checking":
      case "complete":
        return;
    }
  }

  async function handleFilePicked(path: string): Promise<void> {
    const preview = await deps.readGnuCashFile(path);
    if (preview.non_usd_accounts.length > 0) {
      const accountList = preview.non_usd_accounts.join(", ");
      deps.addSystemMessage(
        `Your book contains non-USD accounts (${accountList}). Only USD books are supported. Please export a USD-only book and try again.`,
        "error",
      );
      // Stay at gnucash_import_pick_file
      return;
    }
    store.getState().setGnuCashPickedPath(path);
    const plan = await deps.gnucashBuildDefaultPlan(path);
    deps.addGnuCashMappingMessage(plan);
    store.getState().setPhase("gnucash_import_mapping");
  }

  async function handleConfirmMapping(): Promise<void> {
    store.getState().setPhase("gnucash_import_committing");
    let receipt;
    try {
      receipt = await deps.commitGnuCashImport();
    } catch {
      deps.addSystemMessage(
        "I couldn't commit the import. Your data hasn't changed. You can try again, or type 'cancel' to pick a different file.",
        "error",
      );
      store.getState().setPhase("gnucash_import_mapping");
      return;
    }
    store.getState().setGnuCashImportId(receipt.import_id);
    deps.addSystemMessage("Import committed. Checking balances against GnuCash…", "info");
    store.getState().setPhase("gnucash_import_reconciling");
    const pickedPath = store.getState().gnucashPickedPath ?? "";
    try {
      const report = await deps.reconcileGnuCashImport(receipt.import_id, pickedPath);
      deps.addGnuCashReconcileMessage(report);
    } catch {
      deps.addSystemMessage(
        "Import committed, but I couldn't check the balances against GnuCash. You can keep the import by typing 'continue', or type 'rollback' to undo it.",
        "error",
      );
    }
  }

  async function handleAcceptReconcile(): Promise<void> {
    const { householdName, accounts, envelopes } = store.getState().draft;
    deps.addHandoffMessage(
      householdName || "Your household",
      accounts.length,
      envelopes.length,
      STARTER_PROMPTS,
    );
    store.getState().setPhase("gnucash_import_done");
  }

  async function handleRollbackReconcile(): Promise<void> {
    const importId = store.getState().gnucashImportId;
    if (importId) {
      await deps.rollbackGnuCashImport(importId);
    }
    store.getState().setGnuCashImportId(null);
    store.getState().setGnuCashPickedPath(null);
    deps.addSystemMessage(
      "Import rolled back. Pick a GnuCash file to try again, or skip migration.",
      "info",
    );
    store.getState().setPhase("gnucash_import_pick_file");
  }

  function phase() {
    return store.getState().phase;
  }

  return { checkAndStart, handleInput, handleFilePicked, handleConfirmMapping, handleAcceptReconcile, handleRollbackReconcile, phase };
}

export function useOnboardingEngine() {
  const addSystemMessage = useChatStore((s) => s.addSystemMessage);
  const addSetupCard = useChatStore((s) => s.addSetupCard);
  const addHandoffMessage = useChatStore((s) => s.addHandoffMessage);
  const addGnuCashMappingMessage = useChatStore((s) => s.addGnuCashMappingMessage);
  const addGnuCashReconcileMessage = useChatStore((s) => s.addGnuCashReconcileMessage);
  const phase = useOnboardingStore((s) => s.phase);
  const invalidateSidebar = useInvalidateSidebar();

  const readGnuCashFile = useCallback(
    (path: string) => invokeOrThrow<GnuCashPreview>(undefined, "read_gnucash_file", { path }),
    [],
  );
  const gnucashBuildDefaultPlan = useCallback(
    (path: string) => invokeOrThrow<ImportPlan>(undefined, "gnucash_build_default_plan", { path }),
    [],
  );
  const gnucashApplyMappingEdit = useCallback(
    (edit: MappingEdit) => invokeOrThrow<ImportPlan>(undefined, "gnucash_apply_mapping_edit", { edit }),
    [],
  );
  const commitGnuCashImport = useCallback(
    () => invokeOrThrow<ImportReceipt>(undefined, "gnucash_commit_import", {}),
    [],
  );
  const reconcileGnuCashImport = useCallback(
    (importId: string, path: string) =>
      invokeOrThrow<GnuCashReconcileReport>(undefined, "reconcile_gnucash_import", { import_id: importId, path }),
    [],
  );
  const rollbackGnuCashImport = useCallback(
    (importId: string) => invokeOrThrow<void>(undefined, "rollback_gnucash_import", { import_id: importId }),
    [],
  );

  const deps: OnboardingDeps = useMemo(
    () => ({
      addSystemMessage,
      addSetupCard,
      addHandoffMessage,
      addGnuCashMappingMessage,
      addGnuCashReconcileMessage,
      invalidateSidebar,
      readGnuCashFile,
      gnucashBuildDefaultPlan,
      gnucashApplyMappingEdit,
      commitGnuCashImport,
      reconcileGnuCashImport,
      rollbackGnuCashImport,
    }),
    [
      addSystemMessage,
      addSetupCard,
      addHandoffMessage,
      addGnuCashMappingMessage,
      addGnuCashReconcileMessage,
      invalidateSidebar,
      readGnuCashFile,
      gnucashBuildDefaultPlan,
      gnucashApplyMappingEdit,
      commitGnuCashImport,
      reconcileGnuCashImport,
      rollbackGnuCashImport,
    ],
  );

  const handler = useMemo(() => buildOnboardingHandler(deps), [deps]);

  useEffect(() => {
    void handler.checkAndStart();
  }, []);

  return {
    isActive: phase !== "complete",
    handleInput: handler.handleInput,
    handleFilePicked: handler.handleFilePicked,
    handleConfirmMapping: handler.handleConfirmMapping,
    handleAcceptReconcile: handler.handleAcceptReconcile,
    handleRollbackReconcile: handler.handleRollbackReconcile,
  };
}
