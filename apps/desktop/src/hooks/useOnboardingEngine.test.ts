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

const mockInvoke = vi.mocked(invoke);

function makeDeps(overrides: Partial<OnboardingDeps> = {}): OnboardingDeps {
  return {
    addSystemMessage: vi.fn(),
    addSetupCard: vi.fn(),
    addHandoffMessage: vi.fn(),
    invoke: mockInvoke,
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

  it("completes onboarding on 'no' and shows handoff", async () => {
    const addHandoffMessage = vi.fn();
    const handler = buildOnboardingHandler(makeDeps({ addHandoffMessage }));
    await handler.handleInput("no");
    expect(addHandoffMessage).toHaveBeenCalledWith(
      "Smith Family",
      1,
      1,
      expect.arrayContaining([expect.any(String)]),
    );
    expect(useOnboardingStore.getState().phase).toBe("complete");
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
