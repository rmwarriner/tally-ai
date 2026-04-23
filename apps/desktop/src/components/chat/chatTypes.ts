import type {
  TransactionCardState,
  TransactionDisplay,
} from "./TransactionCard.types";

import type { SetupCardVariant } from "../onboarding/SetupCard";

export type ChatMessage =
  | { kind: "user"; id: string; ts: number; text: string }
  | { kind: "ai"; id: string; ts: number; text: string; model?: string }
  | { kind: "proactive"; id: string; ts: number; text: string; advisory_code?: string }
  | { kind: "system"; id: string; ts: number; text: string; tone?: "info" | "error" }
  | {
      kind: "transaction";
      id: string;
      ts: number;
      transaction_id: string;
      state?: TransactionCardState;
      transaction?: TransactionDisplay;
      replacement?: TransactionDisplay;
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
    };
