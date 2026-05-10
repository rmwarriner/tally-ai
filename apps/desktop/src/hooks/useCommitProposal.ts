import type { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useCallback } from "react";

import type { TransactionProposal } from "../components/chat/chatTypes";
import { safeInvoke } from "../lib/safeInvoke";
import { useChatStore, type ProactiveInsight } from "../stores/chatStore";
import { useInvalidateSidebar } from "./useInvalidateSidebar";

type CommitOutcome =
  | {
      status: "committed";
      txn_id: string;
      /// Optional in JSON; absent or empty when no triggers fired.
      proactive_insights?: ProactiveInsight[];
    }
  | { status: "rejected"; validation: unknown };

/// Mirrors the Rust `AiUsage` shape (T-069). Echoed back from
/// `useSendMessage`'s proposal response so commit_proposal can stamp
/// `ai_input_tokens` / `ai_output_tokens` / `ai_cache_hit` on the row.
export interface AiUsage {
  input_tokens: number;
  output_tokens: number;
  cache_hit: boolean;
}

export interface CommitProposalDeps {
  invoke?: typeof tauriInvoke;
}

/// Returns `{ commit, discard }`. `commit(messageId, proposal)` posts the
/// transaction and flips the card to `posted` state on success; `discard`
/// removes the pending card entirely.
export function useCommitProposal(deps: CommitProposalDeps = {}) {
  const updateMessage = useChatStore((s) => s.updateMessage);
  const removeMessage = useChatStore((s) => s.removeMessage);
  const addSystemMessage = useChatStore((s) => s.addSystemMessage);
  const appendInsight = useChatStore((s) => s.appendInsight);
  const invalidateSidebar = useInvalidateSidebar();

  const commit = useCallback(
    async (messageId: string, proposal: TransactionProposal, ai_usage?: AiUsage) => {
      updateMessage(messageId, { commit_error: undefined });

      const r = await safeInvoke<CommitOutcome>(
        "commit_proposal",
        { args: { proposal, ai_usage: ai_usage ?? null } },
        { invoke: deps.invoke },
      );
      if (!r.ok) {
        const detail = r.error.message;
        updateMessage(messageId, { commit_error: detail });
        addSystemMessage(`Couldn't save that transaction: ${detail}`, "error");
        return;
      }
      const outcome = r.value;

      if (outcome.status === "committed") {
        updateMessage(messageId, {
          state: "posted",
          proposal: undefined,
          commit_error: undefined,
          transaction_id: outcome.txn_id,
        });
        void invalidateSidebar();
        for (const insight of outcome.proactive_insights ?? []) {
          appendInsight(insight);
        }
        return;
      }

      // Rejected by validation — keep the card pending, surface the error.
      const summary = summarizeValidation(outcome.validation);
      updateMessage(messageId, { commit_error: summary });
      addSystemMessage(summary, "error");
    },
    [addSystemMessage, appendInsight, deps, invalidateSidebar, updateMessage],
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
