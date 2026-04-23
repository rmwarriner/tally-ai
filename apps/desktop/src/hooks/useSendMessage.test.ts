import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore } from "../stores/chatStore";
import { useSendMessage } from "./useSendMessage";

beforeEach(() => {
  useChatStore.setState({ localMessages: [] });
});

describe("useSendMessage", () => {
  it("appends the user message and the AI text response for text responses", async () => {
    const invoke = vi.fn(async (cmd: string) => {
      if (cmd === "submit_message") {
        return { kind: "text", text: "your Checking has $1,000" };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    const { result } = renderHook(() => useSendMessage({ invoke: invoke as never }));

    await act(async () => {
      await result.current("what's my balance?");
    });

    const messages = useChatStore.getState().localMessages;
    expect(messages).toHaveLength(2);
    expect(messages[0]).toMatchObject({ kind: "user", text: "what's my balance?" });
    expect(messages[1]).toMatchObject({ kind: "ai", text: "your Checking has $1,000" });
  });

  it("renders a pending transaction card for proposal responses", async () => {
    const invoke = vi.fn(async (cmd: string) => {
      if (cmd === "submit_message") {
        return {
          kind: "proposal",
          proposal: {
            memo: "Coffee",
            txn_date_ms: 1_700_000_000_000,
            lines: [
              { account_id: "acc_grc", amount_cents: 450, side: "debit" },
              { account_id: "acc_chk", amount_cents: 450, side: "credit" },
            ],
          },
          validation: { status: "ACCEPTED" },
          advisories: [],
          account_names: { acc_grc: "Groceries", acc_chk: "Checking" },
        };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    const { result } = renderHook(() => useSendMessage({ invoke: invoke as never }));

    await act(async () => {
      await result.current("I spent $4.50 on coffee");
    });

    const messages = useChatStore.getState().localMessages;
    expect(messages).toHaveLength(2);
    expect(messages[0]).toMatchObject({ kind: "user" });
    expect(messages[1]).toMatchObject({
      kind: "transaction",
      state: "pending",
      transaction: {
        payee: "Coffee",
        amount_cents: 450,
        account_name: "Groceries",
      },
    });
  });

  it("surfaces invoke errors as system messages", async () => {
    const invoke = vi.fn(async () => {
      throw new Error("No Claude API key configured.");
    });

    const { result } = renderHook(() => useSendMessage({ invoke: invoke as never }));

    await act(async () => {
      await result.current("I spent $10");
    });

    const messages = useChatStore.getState().localMessages;
    expect(messages).toHaveLength(2);
    expect(messages[0]).toMatchObject({ kind: "user" });
    expect(messages[1]).toMatchObject({
      kind: "system",
      text: expect.stringContaining("API key"),
      tone: "error",
    });
  });
});
