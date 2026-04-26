import type {
  TransactionCardState,
  TransactionDisplay,
} from "./TransactionCard.types";

import type { SetupCardVariant } from "../onboarding/SetupCard";
import type { ImportPlan, RecoveryAction } from "@tally/core-types";
import type { GnuCashReconcileReport } from "../artifacts/GnuCashReconcileCard";

export interface ProposedLine {
  account_id: string;
  envelope_id?: string | null;
  amount_cents: number;
  side: "debit" | "credit";
}

export interface TransactionProposal {
  memo?: string | null;
  txn_date_ms: number;
  lines: ProposedLine[];
}

export type ChatMessage =
  | { kind: "user"; id: string; ts: number; text: string }
  | { kind: "ai"; id: string; ts: number; text: string; model?: string }
  | {
      kind: "proactive";
      id: string;
      ts: number;
      text: string;
      advisory_code?: string;
      /// Optional recovery actions surfaced when this proactive message
      /// originated from a `RecoveryError` (e.g. via `appendAdvisory`).
      recovery?: RecoveryAction[];
    }
  | { kind: "system"; id: string; ts: number; text: string; tone?: "info" | "error" }
  | {
      kind: "transaction";
      id: string;
      ts: number;
      transaction_id: string;
      state?: TransactionCardState;
      transaction?: TransactionDisplay;
      replacement?: TransactionDisplay;
      /// Present for fresh AI proposals awaiting user confirmation.
      /// Cleared once the proposal is committed or discarded.
      proposal?: TransactionProposal;
      /// Present after a failed commit; clears when the user retries.
      commit_error?: string;
    }
  | {
      kind: "artifact";
      id: string;
      ts: number;
      artifact_id: string;
      title: string;
      content?: string;
    }
  | {
      kind: "setup_card";
      id: string;
      ts: number;
      variant: SetupCardVariant;
      title: string;
      detail: string;
    }
  | {
      kind: "handoff";
      id: string;
      ts: number;
      householdName: string;
      accountCount: number;
      envelopeCount: number;
      starterPrompts: string[];
    }
  | {
      kind: "gnucash_mapping";
      id: string;
      ts: number;
      plan: ImportPlan;
    }
  | {
      kind: "gnucash_reconcile";
      id: string;
      ts: number;
      report: GnuCashReconcileReport;
    };
