import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock Tauri invoke before any imports
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { useOnboardingStore } from "../stores/onboardingStore";
import { useChatStore } from "../stores/chatStore";
import {
  buildOnboardingHandler,
  type OnboardingDeps,
} from "./useOnboardingEngine";
import type { GnuCashPreview, ImportPlan } from "@tally/core-types";

const mockInvoke = vi.mocked(invoke);

const MOCK_PREVIEW: GnuCashPreview = {
  book_guid: "b1",
  account_count: 3,
  transaction_count: 2,
  non_usd_accounts: [],
};

const MOCK_PLAN: ImportPlan = {
  household_id: "hh",
  import_id: "imp",
  account_mappings: [
    {
      gnc_guid: "a",
      gnc_full_name: "Checking",
      tally_account_id: "u1",
      tally_name: "Checking",
      tally_parent_id: null,
      tally_type: "asset",
      tally_normal_balance: "debit",
    },
  ],
  transactions: [],
};

function makeDeps(overrides: Partial<OnboardingDeps> = {}): OnboardingDeps {
  return {
    addSystemMessage: vi.fn(),
    addSetupCard: vi.fn(),
    addHandoffMessage: vi.fn(),
    addGnuCashMappingMessage: vi.fn(),
    invoke: mockInvoke,
    invalidateSidebar: vi.fn(),
    readGnuCashFile: vi.fn().mockResolvedValue(MOCK_PREVIEW),
    gnucashBuildDefaultPlan: vi.fn().mockResolvedValue(MOCK_PLAN),
    gnucashApplyMappingEdit: vi.fn().mockResolvedValue(MOCK_PLAN),
    commitGnuCashImport: vi.fn().mockResolvedValue({
      import_id: "imp",
      accounts_created: 3,
      transactions_committed: 2,
      transactions_skipped: 0,
    }),
    ...overrides,
  };
}

beforeEach(() => {
  useOnboardingStore.setState(useOnboardingStore.getInitialState());
  useChatStore.setState({ localMessages: [] } as never);
  mockInvoke.mockReset();
});

describe("checkAndStart", () => {
  it("sets phase to path_select when no household exists", async () => {
    mockInvoke.mockResolvedValue(false);
    const deps = makeDeps();
    const handler = buildOnboardingHandler(deps);
    await handler.checkAndStart();
    expect(useOnboardingStore.getState().phase).toBe("path_select");
  });

  it("posts a welcome message when onboarding begins", async () => {
    mockInvoke.mockResolvedValue(false);
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.checkAndStart();
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("Welcome"),
      "info",
    );
  });

  it("sets phase to complete when household already exists", async () => {
    mockInvoke.mockResolvedValue(true);
    const handler = buildOnboardingHandler(makeDeps());
    await handler.checkAndStart();
    expect(useOnboardingStore.getState().phase).toBe("complete");
  });
});

describe("path selection", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("path_select");
  });

  it("enters fresh_start on 'fresh'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("fresh");
    expect(useOnboardingStore.getState().phase).toBe("fresh_start");
  });

  it("enters fresh_start on 'start fresh'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("start fresh");
    expect(useOnboardingStore.getState().phase).toBe("fresh_start");
  });

  it("enters migration on 'import'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("import");
    expect(useOnboardingStore.getState().phase).toBe("migration");
  });

  it("enters migration on 'hledger'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("my hledger journal");
    expect(useOnboardingStore.getState().phase).toBe("migration");
  });

  it("posts clarification on unrecognized input", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("maybe");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("fresh"),
      "info",
    );
    expect(useOnboardingStore.getState().phase).toBe("path_select");
  });
});

describe("fresh start — household name step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("household_name");
  });

  it("stores household name and advances to timezone step", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("Smith Family");
    expect(useOnboardingStore.getState().draft.householdName).toBe("Smith Family");
    expect(useOnboardingStore.getState().freshStep).toBe("timezone");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("timezone"),
      "info",
    );
  });

  it("rejects empty household name", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("   ");
    expect(useOnboardingStore.getState().freshStep).toBe("household_name");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("name"),
      "info",
    );
  });
});

describe("fresh start — timezone step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("timezone");
  });

  it("stores timezone and advances to passphrase step", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("America/Chicago");
    expect(useOnboardingStore.getState().draft.timezone).toBe("America/Chicago");
    expect(useOnboardingStore.getState().freshStep).toBe("passphrase");
  });
});

describe("fresh start — passphrase step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("passphrase");
  });

  it("stores passphrase and advances to confirm step", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("my-secret-phrase");
    expect(useOnboardingStore.getState().draft.passphrase).toBe("my-secret-phrase");
    expect(useOnboardingStore.getState().freshStep).toBe("confirm_passphrase");
  });
});

describe("fresh start — confirm passphrase step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("confirm_passphrase");
    useOnboardingStore.getState().patchDraft({
      householdName: "Smith Family",
      timezone: "America/Chicago",
      passphrase: "my-secret-phrase",
    });
  });

  it("creates household when passphrase matches", async () => {
    mockInvoke.mockResolvedValue("hh_01");
    const addSetupCard = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSetupCard }));
    await handler.handleInput("my-secret-phrase");
    expect(mockInvoke).toHaveBeenCalledWith("create_household", expect.objectContaining({
      name: "Smith Family",
      timezone: "America/Chicago",
      passphrase: "my-secret-phrase",
    }));
    expect(addSetupCard).toHaveBeenCalledWith("household_created", expect.any(String), expect.any(String));
    expect(useOnboardingStore.getState().freshStep).toBe("accounts");
  });

  it("asks user to retry on passphrase mismatch", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("wrong-phrase");
    expect(useOnboardingStore.getState().freshStep).toBe("passphrase");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("match"),
      "info",
    );
  });
});

describe("fresh start — accounts step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("accounts");
    useOnboardingStore.getState().patchDraft({ householdName: "Smith Family" });
  });

  it("stores account name and advances to account_balance", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("Chase Checking");
    expect(useOnboardingStore.getState().draft.accounts).toHaveLength(1);
    expect(useOnboardingStore.getState().draft.accounts[0].name).toBe("Chase Checking");
    expect(useOnboardingStore.getState().freshStep).toBe("account_balance");
  });
});

describe("fresh start — account_balance step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("account_balance");
    useOnboardingStore.getState().addDraftAccount({ name: "Chase Checking", type: "asset", balanceCents: 0 });
    mockInvoke.mockResolvedValue("acct_01");
  });

  it("parses dollar amount and creates account", async () => {
    const addSetupCard = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSetupCard }));
    await handler.handleInput("$1,500.00");
    expect(useOnboardingStore.getState().draft.accounts[0].balanceCents).toBe(150000);
    expect(mockInvoke).toHaveBeenCalledWith("create_account", expect.any(Object));
    expect(addSetupCard).toHaveBeenCalledWith("account_created", expect.any(String), expect.any(String));
    expect(addSetupCard).toHaveBeenCalledWith("opening_balance", expect.any(String), expect.any(String));
    expect(useOnboardingStore.getState().freshStep).toBe("more_accounts");
  });

  it("parses bare number without dollar sign", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("1500");
    expect(useOnboardingStore.getState().draft.accounts[0].balanceCents).toBe(150000);
  });

  it("asks again on unparseable balance", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("lots of money");
    expect(addSystemMessage).toHaveBeenCalledWith(expect.stringContaining("balance"), "info");
    expect(useOnboardingStore.getState().freshStep).toBe("account_balance");
  });
});

describe("fresh start — more_accounts step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("more_accounts");
  });

  it("loops back to accounts step on 'yes'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("yes");
    expect(useOnboardingStore.getState().freshStep).toBe("accounts");
  });

  it("advances to envelopes on 'no'", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("no");
    expect(useOnboardingStore.getState().freshStep).toBe("envelopes");
    expect(addSystemMessage).toHaveBeenCalledWith(expect.stringContaining("envelope"), "info");
  });

  it("advances to envelopes on 'done'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("done");
    expect(useOnboardingStore.getState().freshStep).toBe("envelopes");
  });
});

describe("fresh start — envelopes step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("envelopes");
    mockInvoke.mockResolvedValue("env_01");
  });

  it("creates envelope and advances to more_envelopes", async () => {
    const addSetupCard = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSetupCard }));
    await handler.handleInput("Groceries");
    expect(useOnboardingStore.getState().draft.envelopes).toHaveLength(1);
    expect(mockInvoke).toHaveBeenCalledWith("create_envelope", expect.any(Object));
    expect(addSetupCard).toHaveBeenCalledWith("envelope_created", expect.any(String), expect.any(String));
    expect(useOnboardingStore.getState().freshStep).toBe("more_envelopes");
  });
});

describe("fresh start — more_envelopes step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("more_envelopes");
    useOnboardingStore.getState().patchDraft({ householdName: "Smith Family" });
    useOnboardingStore.getState().addDraftAccount({ name: "Checking", type: "asset", balanceCents: 0 });
    useOnboardingStore.getState().addDraftEnvelope({ name: "Groceries" });
  });

  it("loops back to envelopes on 'yes'", async () => {
    const handler = buildOnboardingHandler(makeDeps());
    await handler.handleInput("yes");
    expect(useOnboardingStore.getState().freshStep).toBe("envelopes");
  });

  it("advances to api_key step on 'no'", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("no");
    expect(useOnboardingStore.getState().freshStep).toBe("api_key");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("Claude API key"),
      "info",
    );
  });
});

describe("fresh start — api_key step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
    useOnboardingStore.getState().setFreshStep("api_key");
    useOnboardingStore.getState().patchDraft({ householdName: "Smith Family" });
    useOnboardingStore.getState().addDraftAccount({ name: "Checking", type: "asset", balanceCents: 0 });
    useOnboardingStore.getState().addDraftEnvelope({ name: "Groceries" });
  });

  it("saves the key and completes onboarding", async () => {
    mockInvoke.mockResolvedValue(undefined);
    const addHandoffMessage = vi.fn();
    const addSetupCard = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addHandoffMessage, addSetupCard }));
    await handler.handleInput("sk-ant-api03-abc123");
    expect(mockInvoke).toHaveBeenCalledWith("set_api_key", { key: "sk-ant-api03-abc123" });
    expect(addSetupCard).toHaveBeenCalledWith(
      "household_created",
      "API key saved",
      expect.stringContaining("keychain"),
    );
    expect(addHandoffMessage).toHaveBeenCalledWith("Smith Family", 1, 1, expect.any(Array));
    expect(useOnboardingStore.getState().phase).toBe("complete");
  });

  it("skips the key on 'skip' and still completes onboarding", async () => {
    const addSystemMessage = vi.fn();
    const addHandoffMessage = vi.fn();
    const handler = buildOnboardingHandler(
      makeDeps({ addSystemMessage, addHandoffMessage }),
    );
    await handler.handleInput("skip");
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("later"),
      "info",
    );
    expect(addHandoffMessage).toHaveBeenCalled();
    expect(useOnboardingStore.getState().phase).toBe("complete");
  });

  it("surfaces an error when set_api_key fails and stays on the step", async () => {
    mockInvoke.mockRejectedValue(new Error("kaboom"));
    const addSystemMessage = vi.fn();
    const addHandoffMessage = vi.fn();
    const handler = buildOnboardingHandler(
      makeDeps({ addSystemMessage, addHandoffMessage }),
    );
    await handler.handleInput("sk-ant-bad");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("kaboom"),
      "error",
    );
    expect(addHandoffMessage).not.toHaveBeenCalled();
    expect(useOnboardingStore.getState().freshStep).toBe("api_key");
  });

  it("re-prompts on empty input", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("   ");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("skip"),
      "info",
    );
    expect(useOnboardingStore.getState().freshStep).toBe("api_key");
  });
});

describe("migration path", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("migration");
    useOnboardingStore.getState().setMigrationStep("welcome");
  });

  it("advances to file_drop step and posts instructions", async () => {
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("any input triggers file_drop prompt");
    expect(useOnboardingStore.getState().migrationStep).toBe("file_drop");
    expect(addSystemMessage).toHaveBeenCalledWith(expect.stringContaining("journal"), "info");
  });

  it("imports hledger content and advances to coa_mapping", async () => {
    useOnboardingStore.getState().setMigrationStep("file_drop");
    mockInvoke.mockResolvedValue("3 accounts, 42 transactions imported");
    const addSystemMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSystemMessage }));
    await handler.handleInput("; hledger journal content\n2024-01-01 Opening\n  assets:checking  $1000\n  equity:opening  $-1000");
    expect(mockInvoke).toHaveBeenCalledWith("import_hledger", expect.any(Object));
    expect(useOnboardingStore.getState().migrationStep).toBe("coa_mapping");
  });

  it("completes migration after coa_mapping", async () => {
    useOnboardingStore.getState().setMigrationStep("coa_mapping");
    useOnboardingStore.getState().patchDraft({ householdName: "Imported" });
    const addHandoffMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addHandoffMessage }));
    await handler.handleInput("looks good");
    expect(addHandoffMessage).toHaveBeenCalled();
    expect(useOnboardingStore.getState().phase).toBe("complete");
  });
});

describe("GnuCash migration branch — Task 15: intent detection", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("path_select");
  });

  it("detects 'migrate from gnucash' intent and emits file-picker setup card", async () => {
    const addSetupCard = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSetupCard }));
    await handler.handleInput("I'd like to migrate from GnuCash");
    expect(addSetupCard).toHaveBeenCalledWith("gnucash_file_picker", expect.any(String), expect.any(String));
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_pick_file");
  });

  it("detects standalone 'gnucash' keyword and emits file-picker setup card", async () => {
    const addSetupCard = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addSetupCard }));
    await handler.handleInput("I use gnucash");
    expect(addSetupCard).toHaveBeenCalledWith("gnucash_file_picker", expect.any(String), expect.any(String));
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_pick_file");
  });
});

describe("GnuCash import store state persistence", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("path_select");
  });

  it("gnucashPickedPath is set in store after handleFilePicked", async () => {
    const deps = makeDeps();
    const handler = buildOnboardingHandler(deps);
    await handler.handleInput("migrate from GnuCash");
    await handler.handleFilePicked("/tmp/book.gnucash");
    expect(useOnboardingStore.getState().gnucashPickedPath).toBe("/tmp/book.gnucash");
  });

  it("state persists across a simulated re-render (two handlers from same store)", async () => {
    const deps = makeDeps();
    // First handler picks the file
    const handler1 = buildOnboardingHandler(deps);
    await handler1.handleInput("migrate from GnuCash");
    await handler1.handleFilePicked("/tmp/book.gnucash");
    expect(useOnboardingStore.getState().gnucashPickedPath).toBe("/tmp/book.gnucash");
    // Simulate re-render: construct a fresh handler from the same store
    const handler2 = buildOnboardingHandler(deps);
    // The path should still be in the store
    expect(useOnboardingStore.getState().gnucashPickedPath).toBe("/tmp/book.gnucash");
    // And the second handler can confirm the mapping (store phase is gnucash_import_mapping)
    await handler2.handleConfirmMapping();
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_reconciling");
  });
});

describe("GnuCash migration branch — Task 16: file picker flow", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("path_select");
  });

  it("after picking a valid USD book, transitions to mapping phase with default plan", async () => {
    const addGnuCashMappingMessage = vi.fn();
    const deps = makeDeps({ addGnuCashMappingMessage });
    const handler = buildOnboardingHandler(deps);
    await handler.handleInput("migrate from GnuCash");
    await handler.handleFilePicked("/tmp/book.gnucash");
    expect(deps.readGnuCashFile).toHaveBeenCalledWith("/tmp/book.gnucash");
    expect(deps.gnucashBuildDefaultPlan).toHaveBeenCalledWith("/tmp/book.gnucash");
    expect(addGnuCashMappingMessage).toHaveBeenCalledWith(MOCK_PLAN);
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_mapping");
  });

  it("rejects non-USD books with a hard-error system message and stays at pick_file phase", async () => {
    const addSystemMessage = vi.fn();
    const deps = makeDeps({
      addSystemMessage,
      readGnuCashFile: vi.fn().mockResolvedValue({
        book_guid: "b1",
        account_count: 2,
        transaction_count: 0,
        non_usd_accounts: ["Euro Savings"],
      }),
    });
    const handler = buildOnboardingHandler(deps);
    await handler.handleInput("migrate from GnuCash");
    await handler.handleFilePicked("/tmp/book.gnucash");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("Euro Savings"),
      "error",
    );
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_pick_file");
  });

  it("phase() accessor returns current store phase", async () => {
    const deps = makeDeps();
    const handler = buildOnboardingHandler(deps);
    await handler.handleInput("migrate from GnuCash");
    await handler.handleFilePicked("/tmp/book.gnucash");
    expect(handler.phase()).toBe("gnucash_import_mapping");
  });
});

describe("GnuCash migration branch — Task 18: mapping-edit loop", () => {
  async function setupMappingPhase(overrides: Partial<OnboardingDeps> = {}) {
    const deps = makeDeps(overrides);
    const handler = buildOnboardingHandler(deps);
    await handler.handleInput("migrate from GnuCash");
    await handler.handleFilePicked("/tmp/book.gnucash");
    return { deps, handler };
  }

  it("applies a change_type edit when user asks 'make X a liability'", async () => {
    const updatedPlan: ImportPlan = {
      household_id: "hh",
      import_id: "imp",
      account_mappings: [
        {
          gnc_guid: "a",
          gnc_full_name: "Groceries",
          tally_account_id: "u1",
          tally_name: "Groceries",
          tally_parent_id: null,
          tally_type: "liability",
          tally_normal_balance: "credit",
        },
      ],
      transactions: [],
    };
    const applyEdit = vi.fn().mockResolvedValue(updatedPlan);
    const addGnuCashMappingMessage = vi.fn();
    const { handler } = await setupMappingPhase({
      gnucashApplyMappingEdit: applyEdit,
      addGnuCashMappingMessage,
    });

    await handler.handleInput("make Groceries a liability");
    expect(applyEdit).toHaveBeenCalledWith({
      kind: "change_type",
      gnc_full_name: "Groceries",
      new_type: "liability",
      new_normal_balance: "credit",
    });
    expect(addGnuCashMappingMessage).toHaveBeenCalledTimes(2); // once on pick, once after edit
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_mapping");
  });

  it("applies a rename edit when user asks 'rename X to Y'", async () => {
    const applyEdit = vi.fn().mockResolvedValue(MOCK_PLAN);
    const { handler } = await setupMappingPhase({ gnucashApplyMappingEdit: applyEdit });
    await handler.handleInput("rename Groceries to Food");
    expect(applyEdit).toHaveBeenCalledWith({
      kind: "rename",
      gnc_full_name: "Groceries",
      new_tally_name: "Food",
    });
  });

  it("emits info message when mapping edit text is not parseable", async () => {
    const addSystemMessage = vi.fn();
    const { handler } = await setupMappingPhase({ addSystemMessage });
    await handler.handleInput("I want to change something but not sure how");
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("make"),
      "info",
    );
    expect(useOnboardingStore.getState().phase).toBe("gnucash_import_mapping");
  });

  it("confirms the plan and transitions to reconciling phase", async () => {
    const commit = vi.fn().mockResolvedValue({
      import_id: "imp",
      accounts_created: 3,
      transactions_committed: 2,
      transactions_skipped: 0,
    });
    const { handler } = await setupMappingPhase({ commitGnuCashImport: commit });
    await handler.handleConfirmMapping();
    expect(commit).toHaveBeenCalled();
    expect(handler.phase()).toBe("gnucash_import_reconciling");
  });

  it("emits a system message after successful commit", async () => {
    const addSystemMessage = vi.fn();
    const { handler } = await setupMappingPhase({ addSystemMessage });
    await handler.handleConfirmMapping();
    expect(addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("Import committed"),
      "info",
    );
  });

  it("stores the import_id in the onboarding store after commit", async () => {
    const commit = vi.fn().mockResolvedValue({
      import_id: "imp-42",
      accounts_created: 1,
      transactions_committed: 5,
      transactions_skipped: 0,
    });
    const { handler } = await setupMappingPhase({ commitGnuCashImport: commit });
    await handler.handleConfirmMapping();
    expect(useOnboardingStore.getState().gnucashImportId).toBe("imp-42");
  });

  it("rejects article-only names like 'make an asset'", async () => {
    const applyEdit = vi.fn().mockResolvedValue(MOCK_PLAN);
    const addSystemMessage = vi.fn();
    const { handler } = await setupMappingPhase({
      gnucashApplyMappingEdit: applyEdit,
      addSystemMessage,
    });
    await handler.handleInput("make an asset");
    expect(applyEdit).not.toHaveBeenCalled();
    expect(addSystemMessage).toHaveBeenCalledWith(expect.stringContaining("make"), "info");
    await handler.handleInput("make a liability");
    expect(applyEdit).not.toHaveBeenCalled();
  });
});

describe("sidebar invalidation", () => {
  it("calls invalidateSidebar after each DB write", async () => {
    const invalidateSidebar = vi.fn();
    const mockInvokeLocal = vi.fn()
      .mockResolvedValueOnce(false)       // check_setup_status (read, no invalidate)
      .mockResolvedValueOnce("hh_01")     // create_household (write, invalidate)
      .mockResolvedValueOnce("ac_01")     // create_account (write, invalidate)
      .mockResolvedValueOnce(undefined)   // set_opening_balance (write, invalidate)
      .mockResolvedValueOnce("en_01");    // create_envelope (write, invalidate)

    const handler = buildOnboardingHandler(makeDeps({
      invoke: mockInvokeLocal as never,
      invalidateSidebar,
    }));

    await handler.checkAndStart();
    await handler.handleInput("fresh");
    await handler.handleInput("Smith Family");
    await handler.handleInput("America/Chicago");
    await handler.handleInput("correcthorsebatterystaple");
    await handler.handleInput("correcthorsebatterystaple"); // confirm passphrase → create_household
    await handler.handleInput("Checking");
    await handler.handleInput("1000"); // create_account + set_opening_balance
    await handler.handleInput("done"); // more_accounts → envelopes
    await handler.handleInput("Groceries"); // create_envelope

    // One invalidation per write: create_household, create_account,
    // set_opening_balance, create_envelope = 4 writes total for the fresh path.
    expect(invalidateSidebar).toHaveBeenCalledTimes(4);
  });
});
