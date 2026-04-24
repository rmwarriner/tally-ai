import { create } from "zustand";

// Inline subset of AccountType to avoid pulling in the workspace package
type AccountType = "asset" | "liability" | "income" | "expense" | "equity";

export type OnboardingPhase =
  | "checking"
  | "path_select"
  | "fresh_start"
  | "migration"
  | "gnucash_import_pick_file"
  | "gnucash_import_mapping"
  | "gnucash_import_committing"
  | "gnucash_import_reconciling"
  | "gnucash_import_done"
  | "complete";

export type FreshStep =
  | "welcome"
  | "household_name"
  | "timezone"
  | "passphrase"
  | "confirm_passphrase"
  | "accounts"
  | "account_balance"
  | "more_accounts"
  | "envelopes"
  | "more_envelopes"
  | "api_key"
  | "done";

export type MigrationStep = "welcome" | "file_drop" | "coa_mapping" | "done";

export interface DraftAccount {
  name: string;
  type: AccountType;
  balanceCents: number;
}

export interface DraftEnvelope {
  name: string;
}

export interface OnboardingDraft {
  householdName: string;
  timezone: string;
  passphrase: string;
  accounts: DraftAccount[];
  envelopes: DraftEnvelope[];
  hledgerContent?: string;
}

interface OnboardingStore {
  phase: OnboardingPhase;
  freshStep: FreshStep;
  migrationStep: MigrationStep;
  currentAccountIndex: number;
  draft: OnboardingDraft;

  setPhase: (phase: OnboardingPhase) => void;
  setFreshStep: (step: FreshStep) => void;
  setMigrationStep: (step: MigrationStep) => void;
  patchDraft: (patch: Partial<OnboardingDraft>) => void;
  addDraftAccount: (account: DraftAccount) => void;
  addDraftEnvelope: (envelope: DraftEnvelope) => void;
  advanceAccountIndex: () => void;
}

const INITIAL_DRAFT: OnboardingDraft = {
  householdName: "",
  timezone: "",
  passphrase: "",
  accounts: [],
  envelopes: [],
};

function makeInitialState() {
  return {
    phase: "checking" as OnboardingPhase,
    freshStep: "welcome" as FreshStep,
    migrationStep: "welcome" as MigrationStep,
    currentAccountIndex: 0,
    draft: { ...INITIAL_DRAFT, accounts: [], envelopes: [] },
  };
}

export const useOnboardingStore = create<OnboardingStore>((set) => ({
  ...makeInitialState(),

  setPhase: (phase) => {
    set((s) => ({
      phase,
      freshStep: phase === "fresh_start" ? "welcome" : s.freshStep,
      migrationStep: phase === "migration" ? "welcome" : s.migrationStep,
    }));
  },

  setFreshStep: (step) => set({ freshStep: step }),

  setMigrationStep: (step) => set({ migrationStep: step }),

  patchDraft: (patch) =>
    set((s) => ({ draft: { ...s.draft, ...patch } })),

  addDraftAccount: (account) =>
    set((s) => ({ draft: { ...s.draft, accounts: [...s.draft.accounts, account] } })),

  addDraftEnvelope: (envelope) =>
    set((s) => ({ draft: { ...s.draft, envelopes: [...s.draft.envelopes, envelope] } })),

  advanceAccountIndex: () =>
    set((s) => ({ currentAccountIndex: s.currentAccountIndex + 1 })),
}));

// Exported for test resets
export { makeInitialState as getOnboardingInitialState };
