import { beforeEach, describe, expect, it } from "vitest";
import { useOnboardingStore } from "./onboardingStore";

beforeEach(() => {
  useOnboardingStore.setState(useOnboardingStore.getInitialState());
});

describe("phase transitions", () => {
  it("starts in checking phase", () => {
    expect(useOnboardingStore.getState().phase).toBe("checking");
  });

  it("transitions to path_select when no household found", () => {
    useOnboardingStore.getState().setPhase("path_select");
    expect(useOnboardingStore.getState().phase).toBe("path_select");
  });

  it("transitions to fresh_start path", () => {
    useOnboardingStore.getState().setPhase("fresh_start");
    expect(useOnboardingStore.getState().phase).toBe("fresh_start");
    expect(useOnboardingStore.getState().freshStep).toBe("welcome");
  });

  it("transitions to migration path", () => {
    useOnboardingStore.getState().setPhase("migration");
    expect(useOnboardingStore.getState().phase).toBe("migration");
    expect(useOnboardingStore.getState().migrationStep).toBe("welcome");
  });

  it("transitions to complete", () => {
    useOnboardingStore.getState().setPhase("complete");
    expect(useOnboardingStore.getState().phase).toBe("complete");
  });
});

describe("fresh start step progression", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("fresh_start");
  });

  it("advances through each step", () => {
    const steps: Array<typeof useOnboardingStore.getState.prototype> = [
      "household_name",
      "timezone",
      "passphrase",
      "confirm_passphrase",
      "accounts",
      "account_balance",
      "more_accounts",
      "envelopes",
      "more_envelopes",
      "done",
    ];
    for (const step of steps) {
      useOnboardingStore.getState().setFreshStep(step as never);
      expect(useOnboardingStore.getState().freshStep).toBe(step);
    }
  });

  it("stores collected data", () => {
    useOnboardingStore.getState().patchDraft({ householdName: "Smith Family" });
    expect(useOnboardingStore.getState().draft.householdName).toBe("Smith Family");
  });

  it("stores multiple draft fields independently", () => {
    useOnboardingStore.getState().patchDraft({ householdName: "Jones" });
    useOnboardingStore.getState().patchDraft({ timezone: "America/Chicago" });
    const { draft } = useOnboardingStore.getState();
    expect(draft.householdName).toBe("Jones");
    expect(draft.timezone).toBe("America/Chicago");
  });

  it("appends accounts to draft", () => {
    useOnboardingStore.getState().addDraftAccount({ name: "Checking", type: "asset", balanceCents: 150000 });
    useOnboardingStore.getState().addDraftAccount({ name: "Savings", type: "asset", balanceCents: 500000 });
    expect(useOnboardingStore.getState().draft.accounts).toHaveLength(2);
    expect(useOnboardingStore.getState().draft.accounts[0].name).toBe("Checking");
  });

  it("appends envelopes to draft", () => {
    useOnboardingStore.getState().addDraftEnvelope({ name: "Groceries" });
    expect(useOnboardingStore.getState().draft.envelopes).toHaveLength(1);
    expect(useOnboardingStore.getState().draft.envelopes[0].name).toBe("Groceries");
  });

  it("tracks current account index for balance collection", () => {
    useOnboardingStore.getState().addDraftAccount({ name: "Checking", type: "asset", balanceCents: 0 });
    useOnboardingStore.getState().addDraftAccount({ name: "Savings", type: "asset", balanceCents: 0 });
    expect(useOnboardingStore.getState().currentAccountIndex).toBe(0);
    useOnboardingStore.getState().advanceAccountIndex();
    expect(useOnboardingStore.getState().currentAccountIndex).toBe(1);
  });
});

describe("migration step progression", () => {
  beforeEach(() => {
    useOnboardingStore.getState().setPhase("migration");
  });

  it("advances migration steps", () => {
    useOnboardingStore.getState().setMigrationStep("file_drop");
    expect(useOnboardingStore.getState().migrationStep).toBe("file_drop");
    useOnboardingStore.getState().setMigrationStep("coa_mapping");
    expect(useOnboardingStore.getState().migrationStep).toBe("coa_mapping");
    useOnboardingStore.getState().setMigrationStep("done");
    expect(useOnboardingStore.getState().migrationStep).toBe("done");
  });

  it("stores hledger content", () => {
    useOnboardingStore.getState().patchDraft({ hledgerContent: "; sample journal" });
    expect(useOnboardingStore.getState().draft.hledgerContent).toBe("; sample journal");
  });
});
