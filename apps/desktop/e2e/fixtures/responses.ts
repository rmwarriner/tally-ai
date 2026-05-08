// Canned Tauri command responses used by the mock-invoke E2E harness.
//
// Each scenario builds a `MockResponses` object whose keys are command names
// and whose values are either a single value/error or a function deciding the
// response per call. The harness in `e2e/setup.ts` wires this into
// `window.__TAURI_INTERNALS__.invoke` before the React app boots.
//
// Keep response shapes here in sync with the Rust types in
// `apps/desktop/src-tauri/src/core/` and `commands/mod.rs`. The Rust contract
// integration test (`src-tauri/tests/orchestrator_integration.rs`) is the
// belt-and-braces against drift.

export interface MockError {
  /** Truthy means: reject the invoke promise with this RecoveryError shape. */
  __reject: true;
  message: string;
  recovery: Array<{ kind: string; label: string; is_primary: boolean }>;
}

export interface MockSequence<T = unknown> {
  /** Returns each value in order; the last value repeats. */
  __sequence: Array<T | MockError>;
}

export type MockHandler<Result = unknown> = Result | MockError | MockSequence<Result>;

export type MockResponses = Record<string, MockHandler>;

export function sequence<T>(...values: Array<T | MockError>): MockSequence<T> {
  return { __sequence: values };
}

export function rejectWith(message: string, kind = "SHOW_HELP"): MockError {
  return {
    __reject: true,
    message,
    recovery: [{ kind, label: kind === "SHOW_HELP" ? "Get help" : "Discard", is_primary: true }],
  };
}

// ── Onboarding ───────────────────────────────────────────────────────────────

/** No prior household — onboarding kicks off in path_select. */
export const freshDeviceResponses: MockResponses = {
  check_setup_status: false,
  list_chat_messages: [],
  get_account_balances: [],
  get_current_envelope_periods: [],
  get_pending_transactions: [],
  has_api_key: false,
  create_household: "01HHHHHHHHHHHHHHHHHHHHHHHH",
  create_account: "01ACCTAAAAAAAAAAAAAAAAAAAA",
  set_opening_balance: null,
  create_envelope: "01ENVBBBBBBBBBBBBBBBBBBBBB",
  set_api_key: null,
  append_chat_message: null,
};

// ── Entry / commit / undo ────────────────────────────────────────────────────

/** Existing household — App skips straight to the live chat surface. */
export const existingHouseholdResponses: MockResponses = {
  check_setup_status: true,
  list_chat_messages: [],
  get_account_balances: [
    { id: "01CHK", name: "Checking", type: "asset", balance_cents: 150_000 },
  ],
  get_current_envelope_periods: [
    { envelope_id: "01ENVGROC", name: "Groceries", allocated_cents: 50_000, spent_cents: 0 },
  ],
  get_pending_transactions: [],
  has_api_key: true,
  append_chat_message: null,
};

export function proposalResponse(memo: string, amountCents: number, accountId = "01CHK") {
  return {
    kind: "proposal",
    proposal: {
      txn_date_ms: Date.UTC(2026, 4, 8),
      memo,
      lines: [
        { account_id: accountId, envelope_id: null, amount_cents: amountCents, side: "credit" },
        { account_id: "01EXPGROC", envelope_id: "01ENVGROC", amount_cents: amountCents, side: "debit" },
      ],
    },
    validation: { status: "OK", errors: [], warnings: [] },
    advisories: [],
    account_names: { "01CHK": "Checking", "01EXPGROC": "Groceries" },
  };
}

export function committedResponse(txnId = "01TXNCOMMITTED") {
  return { status: "committed", txn_id: txnId };
}

export function rejectedValidationResponse(userMessage = "That account doesn't exist.") {
  return {
    status: "rejected",
    validation: {
      status: "REJECTED",
      errors: [{ user_message: userMessage }],
    },
  };
}

export function textResponse(text: string) {
  return { kind: "text", text };
}
