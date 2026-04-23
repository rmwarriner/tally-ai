import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore } from "../stores/chatStore";
import { getOnboardingInitialState, useOnboardingStore } from "../stores/onboardingStore";
import { useChatPersistence } from "./useChatPersistence";

function makeInvokeMock(rows: Array<{ id: string; kind: string; payload: string; ts: number }>) {
  return vi.fn(async (cmd: string, _args?: unknown) => {
    if (cmd === "list_chat_messages") return rows;
    if (cmd === "append_chat_message") return;
    throw new Error(`unexpected invoke: ${cmd}`);
  });
}

beforeEach(() => {
  useChatStore.setState({ localMessages: [] });
  useOnboardingStore.setState({ ...getOnboardingInitialState() });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useChatPersistence — returning user", () => {
  it("hydrates the chat store from list_chat_messages when onboarding is complete and store is empty", async () => {
    const invoke = makeInvokeMock([
      // Newest-first, as returned by the backend.
      { id: "02", kind: "ai", payload: JSON.stringify({ kind: "ai", id: "02", ts: 2000, text: "second" }), ts: 2000 },
      { id: "01", kind: "user", payload: JSON.stringify({ kind: "user", id: "01", ts: 1000, text: "first" }), ts: 1000 },
    ]);

    useOnboardingStore.setState({ phase: "complete" });

    renderHook(() => useChatPersistence({ invoke: invoke as never }));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const messages = useChatStore.getState().localMessages;
    expect(messages.map((m) => m.id)).toEqual(["01", "02"]);
    expect(invoke).toHaveBeenCalledWith("list_chat_messages", expect.any(Object));
  });

  it("persists new messages added after hydration", async () => {
    const invoke = makeInvokeMock([]);
    useOnboardingStore.setState({ phase: "complete" });

    renderHook(() => useChatPersistence({ invoke: invoke as never }));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // Now add a new message — it should be persisted.
    act(() => {
      useChatStore.getState().addUserMessage("hello");
    });
    await act(async () => {
      await Promise.resolve();
    });

    const appendCalls = invoke.mock.calls.filter(([cmd]) => cmd === "append_chat_message");
    expect(appendCalls).toHaveLength(1);
    expect(appendCalls[0][1]).toMatchObject({ args: { kind: "user" } });
  });
});

describe("useChatPersistence — fresh user", () => {
  it("does not back-write onboarding messages already in the store when hydration runs", async () => {
    const invoke = makeInvokeMock([]);
    // Onboarding has populated a few messages in-memory; phase is not yet complete.
    useOnboardingStore.setState({ phase: "fresh_start" });
    act(() => {
      useChatStore.getState().addSystemMessage("welcome");
      useChatStore.getState().addSystemMessage("what's your household name?");
    });

    renderHook(() => useChatPersistence({ invoke: invoke as never }));

    // Transition to complete.
    act(() => {
      useOnboardingStore.setState({ phase: "complete" });
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // Nothing should be backfilled — those messages are ephemeral onboarding chatter.
    const appendCalls = invoke.mock.calls.filter(([cmd]) => cmd === "append_chat_message");
    expect(appendCalls).toHaveLength(0);
    // And list_chat_messages should NOT have been called, because the store is non-empty.
    const listCalls = invoke.mock.calls.filter(([cmd]) => cmd === "list_chat_messages");
    expect(listCalls).toHaveLength(0);
  });

  it("persists messages added after onboarding completes", async () => {
    const invoke = makeInvokeMock([]);
    useOnboardingStore.setState({ phase: "fresh_start" });
    act(() => {
      useChatStore.getState().addSystemMessage("welcome");
    });

    renderHook(() => useChatPersistence({ invoke: invoke as never }));

    act(() => {
      useOnboardingStore.setState({ phase: "complete" });
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    act(() => {
      useChatStore.getState().addUserMessage("first real message");
    });
    await act(async () => {
      await Promise.resolve();
    });

    const appendCalls = invoke.mock.calls.filter(([cmd]) => cmd === "append_chat_message");
    expect(appendCalls).toHaveLength(1);
    expect(appendCalls[0][1]).toMatchObject({ args: { kind: "user" } });
  });
});

describe("useChatPersistence — inactive phases", () => {
  it("does not invoke either command while onboarding is still in progress", async () => {
    const invoke = makeInvokeMock([]);
    useOnboardingStore.setState({ phase: "path_select" });

    renderHook(() => useChatPersistence({ invoke: invoke as never }));
    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      useChatStore.getState().addSystemMessage("hello");
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(invoke).not.toHaveBeenCalled();
  });
});
