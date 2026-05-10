import type { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useCallback } from "react";

import type { TransactionProposal } from "../components/chat/chatTypes";
import type {
  JournalLineDisplay,
  TransactionDisplay,
} from "../components/chat/TransactionCard.types";
import { safeInvokeOrAdvise } from "../lib/safeInvoke";
import { useChatStore, type ProactiveInsight } from "../stores/chatStore";
import { generateUlid } from "../utils/ulid";

/// Mirrors `ai::adapter::AiUsage`. Echoed back to commit_proposal so the
/// resulting transaction row carries the per-call token totals.
export interface AiUsage {
  input_tokens: number;
  output_tokens: number;
  cache_hit: boolean;
}

type MessageResponse =
  | { kind: "text"; text: string }
  | {
      kind: "proposal";
      proposal: TransactionProposal;
      validation: unknown;
      advisories: unknown[];
      account_names: Record<string, string>;
      /// Optional in JSON; absent or empty when no triggers fired.
      proactive_insights?: ProactiveInsight[];
      /// Optional in JSON; defaulted server-side. Stored on the message
      /// so `useCommitProposal` can echo it back at Confirm time.
      ai_usage?: AiUsage;
    };

export interface SendMessageDeps {
  invoke?: typeof tauriInvoke;
}

export function useSendMessage(deps: SendMessageDeps = {}) {
  const addUserMessage = useChatStore((state) => state.addUserMessage);
  const addLocalMessage = useChatStore((state) => state.addLocalMessage);
  const appendInsight = useChatStore((state) => state.appendInsight);

  return useCallback(
    async (text: string) => {
      addUserMessage(text);

      const response = await safeInvokeOrAdvise<MessageResponse>(
        "submit_message",
        { args: { text } },
        { invoke: deps.invoke },
      );
      if (response === null) return; // advisory already dispatched

      if (response.kind === "text") {
        addLocalMessage({
          kind: "ai",
          id: generateUlid(),
          ts: Date.now(),
          text: response.text,
        });
        return;
      }

      const display = proposalToDisplay(response.proposal, response.account_names);
      addLocalMessage({
        kind: "transaction",
        id: generateUlid(),
        ts: Date.now(),
        transaction_id: display.id,
        state: "pending",
        transaction: display,
        proposal: response.proposal,
        ai_usage: response.ai_usage,
      });
      for (const insight of response.proactive_insights ?? []) {
        appendInsight(insight);
      }
    },
    [addUserMessage, addLocalMessage, appendInsight, deps],
  );
}

function proposalToDisplay(
  proposal: TransactionProposal,
  accountNames: Record<string, string>,
): TransactionDisplay {
  const lines: JournalLineDisplay[] = proposal.lines.map((l) => ({
    account_name: accountNames[l.account_id] ?? l.account_id,
    amount_cents: l.amount_cents,
    side: l.side,
  }));
  const primary = proposal.lines.find((l) => l.side === "debit") ?? proposal.lines[0];
  const totalDebits = proposal.lines
    .filter((l) => l.side === "debit")
    .reduce((sum, l) => sum + l.amount_cents, 0);
  return {
    id: generateUlid(),
    payee: proposal.memo ?? "",
    txn_date: proposal.txn_date_ms,
    amount_cents: totalDebits,
    account_name: accountNames[primary.account_id] ?? primary.account_id,
    lines,
  };
}
