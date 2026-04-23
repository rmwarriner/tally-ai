import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatMessage, TransactionProposal } from "../components/chat/chatTypes";
import { useChatStore } from "../stores/chatStore";
import { useCommitProposal } from "./useCommitProposal";

function seedPendingMessage(id: string, proposal: TransactionProposal): void {
  const message: ChatMessage = {
    kind: "transaction",
    id,
    ts: Date.now(),
    transaction_id: id,
    state: "pending",
    transaction: {
      id,
      payee: "Coffee",
      txn_date: proposal.txn_date_ms,
      amount_cents: 450,
      account_name: "Groceries",
      lines: [],
    },
    proposal,
  };
  useChatStore.setState({ localMessages: [message] });
}

function sampleProposal(): TransactionProposal {
  return {
    memo: "Coffee",
    txn_date_ms: 1_700_000_000_000,
    lines: [
      { account_id: "acc_grc", amount_cents: 450, side: "debit" },
      { account_id: "acc_chk", amount_cents: 450, side: "credit" },
    ],
  };
}

beforeEach(() => {
  useChatStore.setState({ localMessages: [] });
});

describe("useCommitProposal", () => {
  it("flips the message to posted on a committed outcome", async () => {
    seedPendingMessage("msg_1", sampleProposal());
    const invoke = vi.fn(async () => ({ status: "committed", txn_id: "TXN_ABC" }));

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }));
    await act(async () => {
      await result.current.commit("msg_1", sampleProposal());
    });

    const message = useChatStore.getState().localMessages[0] as Extract<
      ChatMessage,
      { kind: "transaction" }
    >;
    expect(message.state).toBe("posted");
    expect(message.proposal).toBeUndefined();
    expect(message.transaction_id).toBe("TXN_ABC");
  });

  it("surfaces the validation error on a rejected outcome", async () => {
    seedPendingMessage("msg_1", sampleProposal());
    const invoke = vi.fn(async () => ({
      status: "rejected",
      validation: {
        status: "REJECTED",
        errors: [{ code: "ZERO_AMOUNT", user_message: "Amounts must be greater than zero." }],
      },
    }));

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }));
    await act(async () => {
      await result.current.commit("msg_1", sampleProposal());
    });

    const messages = useChatStore.getState().localMessages;
    const txn = messages.find((m) => m.kind === "transaction") as Extract<
      ChatMessage,
      { kind: "transaction" }
    >;
    expect(txn.state).toBe("pending");
    expect(txn.commit_error).toContain("Amounts must be greater than zero");

    const systemError = messages.find((m) => m.kind === "system");
    expect(systemError).toMatchObject({ kind: "system", tone: "error" });
  });

  it("surfaces invoke failures as a commit_error and system message", async () => {
    seedPendingMessage("msg_1", sampleProposal());
    const invoke = vi.fn(async () => {
      throw new Error("database locked");
    });

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }));
    await act(async () => {
      await result.current.commit("msg_1", sampleProposal());
    });

    const messages = useChatStore.getState().localMessages;
    const txn = messages.find((m) => m.kind === "transaction") as Extract<
      ChatMessage,
      { kind: "transaction" }
    >;
    expect(txn.commit_error).toContain("database locked");
    expect(messages.some((m) => m.kind === "system" && m.tone === "error")).toBe(true);
  });

  it("discard removes the message from the store", () => {
    seedPendingMessage("msg_1", sampleProposal());
    const invoke = vi.fn();
    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }));

    act(() => {
      result.current.discard("msg_1");
    });

    expect(useChatStore.getState().localMessages).toHaveLength(0);
  });
});
