import type {
  TransactionCardState,
  TransactionDisplay,
} from "./TransactionCard.types";

export type ChatMessage =
  | { kind: "user"; id: string; ts: number; text: string }
  | { kind: "ai"; id: string; ts: number; text: string; model?: string }
  | { kind: "proactive"; id: string; ts: number; text: string }
  | {
      kind: "transaction";
      id: string;
      ts: number;
      transaction_id: string;
      state?: TransactionCardState;
      transaction?: TransactionDisplay;
      replacement?: TransactionDisplay;
    }
  | { kind: "artifact"; id: string; ts: number; artifact_id: string; title: string };
