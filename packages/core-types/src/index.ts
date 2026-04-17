// Shared TypeScript types mirroring Rust structs — T-007
// All monetary values are INTEGER cents. Never use number for money directly.

export type Ulid = string;
export type ISODate = string; // YYYY-MM-DD
export type UnixMs = number;

export type Side = "debit" | "credit";
export type TxnStatus = "pending" | "posted" | "void";
export type TxnSource = "manual" | "ai" | "scheduled" | "import" | "opening_balance";

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

export type RecoveryKind =
  | "CreateMissing"
  | "UseSuggested"
  | "EditField"
  | "PostAnyway"
  | "Discard"
  | "ShowHelp";

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

export interface Account {
  id: Ulid;
  household_id: Ulid;
  parent_id?: Ulid;
  name: string;
  normal_balance: Side;
  is_placeholder: boolean;
  is_active: boolean;
}

export interface Envelope {
  id: Ulid;
  household_id: Ulid;
  account_id: Ulid;
  name: string;
  allocated: number; // INTEGER cents
}

export interface EnvelopePeriod {
  envelope_id: Ulid;
  period_start: UnixMs;
  period_end: UnixMs;
  allocated: number; // INTEGER cents
  spent: number; // INTEGER cents, updated atomically by trigger
}
