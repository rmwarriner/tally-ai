import { describe, it, expect, expectTypeOf } from "vitest";
import type {
  Ulid,
  UnixMs,
  Side,
  AccountType,
  TxnStatus,
  TxnSource,
  UserRole,
  AuditAction,
  RecoveryKind,
  RecoveryAction,
  RecoveryError,
  HardError,
  SoftWarning,
  AIAdvisory,
  ValidationResult,
  ValidationStatus,
  Household,
  User,
  Account,
  Envelope,
  EnvelopePeriod,
  Transaction,
  JournalLine,
  AuditLog,
  ProposedLine,
  TransactionProposal,
} from "./index.js";

// Helpers to verify literal union values at runtime
const SIDES: Side[] = ["debit", "credit"];
const ACCOUNT_TYPES: AccountType[] = ["asset", "liability", "income", "expense", "equity"];
const TXN_STATUSES: TxnStatus[] = ["pending", "posted", "void"];
const TXN_SOURCES: TxnSource[] = ["manual", "ai", "scheduled", "import", "opening_balance"];
const USER_ROLES: UserRole[] = ["owner", "member"];
const AUDIT_ACTIONS: AuditAction[] = ["insert", "update", "delete"];
const RECOVERY_KINDS: RecoveryKind[] = [
  "CREATE_MISSING",
  "USE_SUGGESTED",
  "EDIT_FIELD",
  "POST_ANYWAY",
  "DISCARD",
  "SHOW_HELP",
];
const VALIDATION_STATUSES: ValidationStatus[] = ["approved", "rejected", "pending_confirmation"];

describe("literal union values", () => {
  it("Side covers debit and credit only", () => {
    expect(SIDES).toEqual(["debit", "credit"]);
  });

  it("AccountType covers all five account types", () => {
    expect(ACCOUNT_TYPES).toHaveLength(5);
    expect(ACCOUNT_TYPES).toContain("equity");
  });

  it("TxnStatus covers three statuses", () => {
    expect(TXN_STATUSES).toHaveLength(3);
  });

  it("TxnSource covers all five sources", () => {
    expect(TXN_SOURCES).toHaveLength(5);
  });

  it("UserRole covers owner and member only", () => {
    expect(USER_ROLES).toEqual(["owner", "member"]);
  });

  it("AuditAction covers insert, update, delete", () => {
    expect(AUDIT_ACTIONS).toEqual(["insert", "update", "delete"]);
  });

  it("RecoveryKind uses SCREAMING_SNAKE_CASE matching Rust serde", () => {
    expect(RECOVERY_KINDS).toHaveLength(6);
    expect(RECOVERY_KINDS).toContain("CREATE_MISSING");
    expect(RECOVERY_KINDS).toContain("USE_SUGGESTED");
    expect(RECOVERY_KINDS).toContain("EDIT_FIELD");
    expect(RECOVERY_KINDS).toContain("POST_ANYWAY");
    expect(RECOVERY_KINDS).toContain("DISCARD");
    expect(RECOVERY_KINDS).toContain("SHOW_HELP");
    // Verify PascalCase variants are NOT present
    expect(RECOVERY_KINDS).not.toContain("CreateMissing");
  });

  it("ValidationStatus covers three values", () => {
    expect(VALIDATION_STATUSES).toHaveLength(3);
  });
});

describe("RecoveryAction shape", () => {
  it("requires kind, label, and is_primary", () => {
    const action: RecoveryAction = {
      kind: "CREATE_MISSING",
      label: "Create the missing account",
      is_primary: true,
    };
    expect(action.kind).toBe("CREATE_MISSING");
    expect(action.is_primary).toBe(true);
  });
});

describe("NonEmpty recovery constraint", () => {
  it("HardError.recovery is a tuple with at least one element", () => {
    const primary: RecoveryAction = { kind: "DISCARD", label: "Discard", is_primary: true };
    const err: HardError = {
      code: "UNBALANCED_ENTRY",
      message: "Debits and credits must balance",
      recovery: [primary],
    };
    expect(err.recovery).toHaveLength(1);
    expect(err.recovery[0]).toBe(primary);
  });

  it("SoftWarning.recovery is a tuple with at least one element", () => {
    const primary: RecoveryAction = { kind: "POST_ANYWAY", label: "Post anyway", is_primary: true };
    const warn: SoftWarning = {
      code: "LARGE_AMOUNT",
      message: "Amount is unusually large",
      severity: "medium",
      auto_commit_blocked: false,
      recovery: [primary],
    };
    expect(warn.recovery[0].kind).toBe("POST_ANYWAY");
  });

  it("AIAdvisory.recovery is a tuple with at least one element", () => {
    const primary: RecoveryAction = { kind: "SHOW_HELP", label: "Show help", is_primary: false };
    const advisory: AIAdvisory = {
      code: "LOW_CONFIDENCE",
      message: "Confidence below threshold",
      recovery: [primary],
    };
    expect(advisory.recovery).toHaveLength(1);
  });
});

describe("Household shape", () => {
  it("matches DB schema fields", () => {
    const h: Household = {
      id: "01HQ1111111111111111111111" as Ulid,
      name: "Test Household",
      timezone: "America/Chicago",
      schema_version: 1,
      created_at: 1700000000000 as UnixMs,
    };
    expect(h.timezone).toBe("America/Chicago");
    expect(h.schema_version).toBe(1);
  });
});

describe("User shape", () => {
  it("matches DB schema fields", () => {
    const u: User = {
      id: "01HQ2222222222222222222222" as Ulid,
      household_id: "01HQ1111111111111111111111" as Ulid,
      display_name: "Alice",
      role: "owner",
      is_active: true,
      created_at: 1700000000000 as UnixMs,
    };
    expect(u.role).toBe("owner");
  });
});

describe("Account shape", () => {
  it("includes type and currency, no phantom is_active", () => {
    const a: Account = {
      id: "01HQ3333333333333333333333" as Ulid,
      household_id: "01HQ1111111111111111111111" as Ulid,
      name: "Checking",
      type: "asset",
      normal_balance: "debit",
      is_placeholder: false,
      currency: "USD",
      created_at: 1700000000000 as UnixMs,
    };
    expect(a.type).toBe("asset");
    expect(a.currency).toBe("USD");
    // parent_id is optional — should not be required
    expect(a.parent_id).toBeUndefined();
  });
});

describe("Transaction shape", () => {
  it("matches DB schema fields", () => {
    const t: Transaction = {
      id: "01HQ4444444444444444444444" as Ulid,
      household_id: "01HQ1111111111111111111111" as Ulid,
      txn_date: 1700000000000 as UnixMs,
      entry_date: 1700000000000 as UnixMs,
      status: "posted",
      source: "manual",
      created_at: 1700000000000 as UnixMs,
    };
    expect(t.status).toBe("posted");
    expect(t.memo).toBeUndefined();
  });
});

describe("JournalLine shape", () => {
  it("requires positive amount in cents with side", () => {
    const line: JournalLine = {
      id: "01HQ5555555555555555555555" as Ulid,
      transaction_id: "01HQ4444444444444444444444" as Ulid,
      account_id: "01HQ3333333333333333333333" as Ulid,
      amount: 5000, // $50.00
      side: "debit",
      created_at: 1700000000000 as UnixMs,
    };
    expect(line.amount).toBeGreaterThan(0);
    expect(line.envelope_id).toBeUndefined();
  });
});

describe("Envelope shape", () => {
  it("does not have allocated (that lives on EnvelopePeriod)", () => {
    const e: Envelope = {
      id: "01HQ6666666666666666666666" as Ulid,
      household_id: "01HQ1111111111111111111111" as Ulid,
      account_id: "01HQ3333333333333333333333" as Ulid,
      name: "Groceries",
      created_at: 1700000000000 as UnixMs,
    };
    expect(e.name).toBe("Groceries");
    // Ensure no allocated property bleeds through
    expect((e as unknown as Record<string, unknown>)["allocated"]).toBeUndefined();
  });
});

describe("EnvelopePeriod shape", () => {
  it("includes id, allocated, spent, and created_at", () => {
    const ep: EnvelopePeriod = {
      id: "01HQ7777777777777777777777" as Ulid,
      envelope_id: "01HQ6666666666666666666666" as Ulid,
      period_start: 1700000000000 as UnixMs,
      period_end: 1702677599000 as UnixMs,
      allocated: 30000, // $300.00
      spent: 12500, // $125.00
      created_at: 1700000000000 as UnixMs,
    };
    expect(ep.allocated).toBe(30000);
    expect(ep.spent).toBe(12500);
  });
});

describe("AuditLog shape", () => {
  it("matches DB schema fields", () => {
    const log: AuditLog = {
      id: "01HQ8888888888888888888888" as Ulid,
      household_id: "01HQ1111111111111111111111" as Ulid,
      table_name: "transactions",
      row_id: "01HQ4444444444444444444444",
      action: "insert",
      payload: '{"id":"01HQ4444444444444444444444"}',
      created_at: 1700000000000 as UnixMs,
    };
    expect(log.action).toBe("insert");
    expect(log.user_id).toBeUndefined();
  });
});

describe("TransactionProposal shape", () => {
  it("accepts a valid proposal with balanced lines", () => {
    const primary: RecoveryAction = { kind: "EDIT_FIELD", label: "Edit", is_primary: true };
    const advisory: AIAdvisory = {
      code: "LOW_CONFIDENCE",
      message: "Low confidence",
      recovery: [primary],
    };
    const line: ProposedLine = {
      account_id: "01HQ3333333333333333333333" as Ulid,
      amount: 5000,
      side: "debit",
    };
    const proposal: TransactionProposal = {
      payee: "Whole Foods",
      txn_date: "2024-01-15",
      lines: [line],
      confidence: 0.95,
      confidence_notes: ["payee matched"],
      needs_clarification: false,
      advisories: [advisory],
    };
    expect(proposal.confidence).toBeGreaterThanOrEqual(0);
    expect(proposal.confidence).toBeLessThanOrEqual(1);
  });
});

describe("ValidationResult shape", () => {
  it("approved result has transaction_id", () => {
    const result: ValidationResult = {
      status: "approved",
      transaction_id: "01HQ4444444444444444444444" as Ulid,
      hard_errors: [],
      warnings: [],
      advisories: [],
      confidence: 0.95,
    };
    expect(result.status).toBe("approved");
    expect(result.transaction_id).toBeDefined();
  });

  it("rejected result has hard errors", () => {
    const primary: RecoveryAction = { kind: "EDIT_FIELD", label: "Fix it", is_primary: true };
    const result: ValidationResult = {
      status: "rejected",
      hard_errors: [
        {
          code: "UNBALANCED",
          message: "Debits do not equal credits",
          recovery: [primary],
        },
      ],
      warnings: [],
      advisories: [],
    };
    expect(result.hard_errors).toHaveLength(1);
  });
});

describe("RecoveryError", () => {
  it("has message and non-empty recovery tuple", () => {
    const err: RecoveryError = {
      message: "x",
      recovery: [{ kind: "SHOW_HELP", label: "Help", is_primary: true }],
    };
    expectTypeOf(err.recovery).toMatchTypeOf<[RecoveryAction, ...RecoveryAction[]]>();
  });

  it("recovery tuple cannot be empty (compile-time)", () => {
    // @ts-expect-error - empty array not assignable to non-empty tuple
    const _bad: RecoveryError = { message: "x", recovery: [] };
  });
});
