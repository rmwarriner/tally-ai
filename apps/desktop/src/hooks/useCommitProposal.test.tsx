import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import type { ChatMessage, TransactionProposal } from "../components/chat/chatTypes";
import { useChatStore } from "../stores/chatStore";
import { useCommitProposal } from "./useCommitProposal";

function makeWrapper() {
  const queryClient = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, wrapper };
}

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
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }), {
      wrapper,
    });
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

  it("invalidates the sidebar queries on successful commit", async () => {
    seedPendingMessage("msg_1", sampleProposal());
    const invoke = vi.fn(async () => ({ status: "committed", txn_id: "TXN_ABC" }));
    const { queryClient, wrapper } = makeWrapper();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }), {
      wrapper,
    });

    await act(async () => {
      await result.current.commit("msg_1", sampleProposal());
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["sidebar"] });
  });

  it("does not invalidate on rejection", async () => {
    seedPendingMessage("msg_1", sampleProposal());
    const invoke = vi.fn(async () => ({
      status: "rejected",
      validation: {
        status: "REJECTED",
        errors: [{ user_message: "bad" }],
      },
    }));
    const { queryClient, wrapper } = makeWrapper();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }), {
      wrapper,
    });

    await act(async () => {
      await result.current.commit("msg_1", sampleProposal());
    });

    expect(spy).not.toHaveBeenCalled();
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
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }), {
      wrapper,
    });
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
      // Tauri commands now reject with a serialized RecoveryError shape.
      throw {
        message: "database locked",
        recovery: [{ kind: "SHOW_HELP", label: "Get help", is_primary: true }],
      };
    });
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }), {
      wrapper,
    });
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
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useCommitProposal({ invoke: invoke as never }), {
      wrapper,
    });

    act(() => {
      result.current.discard("msg_1");
    });

    expect(useChatStore.getState().localMessages).toHaveLength(0);
  });
});
