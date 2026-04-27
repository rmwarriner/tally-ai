import type { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useCallback } from "react";

import type { TransactionProposal } from "../components/chat/chatTypes";
import type {
  JournalLineDisplay,
  TransactionDisplay,
} from "../components/chat/TransactionCard.types";
import { safeInvokeOrAdvise } from "../lib/safeInvoke";
import { useChatStore } from "../stores/chatStore";
import { generateUlid } from "../utils/ulid";

type MessageResponse =
  | { kind: "text"; text: string }
  | {
      kind: "proposal";
      proposal: TransactionProposal;
      validation: unknown;
      advisories: unknown[];
      account_names: Record<string, string>;
    };

export interface SendMessageDeps {
  invoke?: typeof tauriInvoke;
}

export function useSendMessage(deps: SendMessageDeps = {}) {
  const addUserMessage = useChatStore((state) => state.addUserMessage);
  const addLocalMessage = useChatStore((state) => state.addLocalMessage);

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
      });
    },
    [addUserMessage, addLocalMessage, deps],
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
