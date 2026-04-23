import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useCallback } from "react";

import type { TransactionProposal } from "../components/chat/chatTypes";
import { useChatStore } from "../stores/chatStore";

type CommitOutcome =
  | { status: "committed"; txn_id: string }
  | { status: "rejected"; validation: unknown };

export interface CommitProposalDeps {
  invoke: typeof tauriInvoke;
}

/// Returns `{ commit, discard }`. `commit(messageId, proposal)` posts the
/// transaction and flips the card to `posted` state on success; `discard`
/// removes the pending card entirely.
export function useCommitProposal(
  deps: CommitProposalDeps = { invoke: tauriInvoke },
) {
  const updateMessage = useChatStore((s) => s.updateMessage);
  const removeMessage = useChatStore((s) => s.removeMessage);
  const addSystemMessage = useChatStore((s) => s.addSystemMessage);

  const commit = useCallback(
    async (messageId: string, proposal: TransactionProposal) => {
      updateMessage(messageId, { commit_error: undefined });

      let outcome: CommitOutcome;
      try {
        outcome = await deps.invoke<CommitOutcome>("commit_proposal", {
          args: { proposal },
        });
      } catch (err) {
        const detail = err instanceof Error ? err.message : String(err);
        updateMessage(messageId, { commit_error: detail });
        addSystemMessage(`Couldn't save that transaction: ${detail}`, "error");
        return;
      }

      if (outcome.status === "committed") {
        updateMessage(messageId, {
          state: "posted",
          proposal: undefined,
          commit_error: undefined,
          transaction_id: outcome.txn_id,
        });
        return;
      }

      // Rejected by validation — keep the card pending, surface the error.
      const summary = summarizeValidation(outcome.validation);
      updateMessage(messageId, { commit_error: summary });
      addSystemMessage(summary, "error");
    },
    [addSystemMessage, deps, updateMessage],
  );

  const discard = useCallback(
    (messageId: string) => {
      removeMessage(messageId);
    },
    [removeMessage],
  );

  return { commit, discard };
}

function summarizeValidation(validation: unknown): string {
  if (!validation || typeof validation !== "object") {
    return "Transaction rejected by validation.";
  }
  const v = validation as { status?: string; errors?: unknown };
  if (v.status === "REJECTED" && Array.isArray(v.errors) && v.errors.length > 0) {
    const first = v.errors[0] as { user_message?: string };
    return first.user_message ?? "Transaction rejected by validation.";
  }
  return "Transaction rejected by validation.";
}
