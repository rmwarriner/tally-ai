// Shared TypeScript types mirroring Rust structs — T-007
// All monetary values are INTEGER cents. Never use number for money directly.

export type Ulid = string;
export type ISODate = string; // YYYY-MM-DD
export type UnixMs = number;

export type Side = "debit" | "credit";
export type AccountType = "asset" | "liability" | "income" | "expense" | "equity";
export type TxnStatus = "pending" | "posted" | "void";
export type TxnSource = "manual" | "ai" | "scheduled" | "import" | "opening_balance";
export type UserRole = "owner" | "member";
export type AuditAction = "insert" | "update" | "delete";

// Mirrors Rust RecoveryKind with #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
export type RecoveryKind =
  | "CREATE_MISSING"
  | "USE_SUGGESTED"
  | "EDIT_FIELD"
  | "POST_ANYWAY"
  | "DISCARD"
  | "SHOW_HELP";

export interface RecoveryAction {
  kind: RecoveryKind;
  label: string;
  is_primary: boolean;
}

export interface HardError {
  code: string;
  message: string;
  field?: string;
  suggestion?: string;
  recovery: [RecoveryAction, ...RecoveryAction[]]; // NonEmpty
}

export interface SoftWarning {
  code: string;
  message: string;
  severity: "low" | "medium" | "high";
  auto_commit_blocked: boolean;
  recovery: [RecoveryAction, ...RecoveryAction[]]; // NonEmpty
}

export interface AIAdvisory {
  code: string;
  message: string;
  suggested_fix?: Partial<TransactionProposal>;
  recovery: [RecoveryAction, ...RecoveryAction[]]; // NonEmpty
}

export interface ProposedLine {
  account_id: Ulid;
  envelope_id?: Ulid;
  amount: number; // INTEGER cents, always positive
  side: Side;
  line_memo?: string;
}

export interface TransactionProposal {
  payee: string;
  txn_date: ISODate;
  memo?: string;
  lines: ProposedLine[];
  confidence: number; // 0.0–1.0
  confidence_notes: string[];
  needs_clarification: boolean;
  clarification_prompt?: string;
  advisories: AIAdvisory[];
}

export type ValidationStatus = "approved" | "rejected" | "pending_confirmation";

export interface ValidationResult {
  status: ValidationStatus;
  transaction_id?: Ulid;
  hard_errors: HardError[];
  warnings: SoftWarning[];
  advisories: AIAdvisory[];
  confidence?: number;
  auto_commit_at?: UnixMs;
}

export interface Household {
  id: Ulid;
  name: string;
  timezone: string; // IANA timezone name
  schema_version: number;
  created_at: UnixMs;
}

export interface User {
  id: Ulid;
  household_id: Ulid;
  display_name: string;
  role: UserRole;
  is_active: boolean;
  created_at: UnixMs;
}

export interface Account {
  id: Ulid;
  household_id: Ulid;
  parent_id?: Ulid;
  name: string;
  type: AccountType;
  normal_balance: Side;
  is_placeholder: boolean;
  currency: string; // ISO 4217, default "USD"
  created_at: UnixMs;
}

export interface Transaction {
  id: Ulid;
  household_id: Ulid;
  txn_date: UnixMs;
  entry_date: UnixMs;
  status: TxnStatus;
  source: TxnSource;
  memo?: string;
  corrects_txn_id?: Ulid;
  ai_confidence?: number; // 0.0–1.0
  ai_prompt_hash?: string; // SHA-256 of prompt
  import_id?: string;
  source_line?: string; // raw input, max 4KB
  created_at: UnixMs;
}

export interface JournalLine {
  id: Ulid;
  transaction_id: Ulid;
  account_id: Ulid;
  envelope_id?: Ulid;
  amount: number; // INTEGER cents, always positive
  side: Side;
  memo?: string;
  created_at: UnixMs;
}

export interface Envelope {
  id: Ulid;
  household_id: Ulid;
  account_id: Ulid;
  name: string;
  created_at: UnixMs;
}

export interface EnvelopePeriod {
  id: Ulid;
  envelope_id: Ulid;
  period_start: UnixMs;
  period_end: UnixMs;
  allocated: number; // INTEGER cents
  spent: number; // INTEGER cents, updated atomically by trigger
  created_at: UnixMs;
}

export interface AuditLog {
  id: Ulid;
  household_id: Ulid;
  table_name: string;
  row_id: string;
  action: AuditAction;
  payload: string; // JSON
  user_id?: Ulid;
  created_at: UnixMs;
}

// ── GnuCash import types ──────────────────────────────────────────────────

export interface GnuCashPreview {
  book_guid: string;
  account_count: number;
  transaction_count: number;
  non_usd_accounts: string[];
}

export type ImportAccountType = "asset" | "liability" | "income" | "expense" | "equity";
export type NormalBalance = "debit" | "credit";
export type JournalSide = "debit" | "credit";

export interface AccountMapping {
  gnc_guid: string;
  gnc_full_name: string;
  tally_account_id: string;
  tally_name: string;
  tally_parent_id: string | null;
  tally_type: ImportAccountType;
  tally_normal_balance: NormalBalance;
}

export interface PlannedLine {
  tally_account_id: string;
  amount_cents: number;
  side: JournalSide;
}

export interface PlannedTransaction {
  gnc_guid: string;
  txn_date: number;
  memo: string | null;
  lines: PlannedLine[];
}

export interface ImportPlan {
  household_id: string;
  import_id: string;
  account_mappings: AccountMapping[];
  transactions: PlannedTransaction[];
}

export type MappingEdit =
  | { kind: "change_type"; gnc_full_name: string; new_type: ImportAccountType; new_normal_balance: NormalBalance }
  | { kind: "rename"; gnc_full_name: string; new_tally_name: string };

export interface ImportReceipt {
  import_id: string;
  accounts_created: number;
  transactions_committed: number;
  transactions_skipped: number;
}
