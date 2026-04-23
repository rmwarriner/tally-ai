import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore } from "../stores/chatStore";
import { UNKNOWN_COMMAND_MESSAGE, dispatchSlashCommand, useSlashDispatch } from "./useSlashDispatch";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

function makeDeps(overrides: Partial<Parameters<typeof dispatchSlashCommand>[2]> = {}) {
  return {
    sendMessage: vi.fn(),
    addSystemMessage: vi.fn(),
    addArtifactMessage: vi.fn(),
    undoLastTransaction: vi.fn().mockResolvedValue(undefined),
    getAIDefaults: vi.fn().mockResolvedValue({ timezone: "America/Chicago", preferred_accounts: ["Checking"] }),
    ...overrides,
  };
}

describe("dispatchSlashCommand", () => {
  it("/budget sends budget prompt via sendMessage", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/budget", "", deps);

    expect(deps.sendMessage).toHaveBeenCalledWith(
      "Show envelope budget status for the current month",
    );
  });

  it("/recent uses argument count when provided", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/recent", "20", deps);

    expect(deps.sendMessage).toHaveBeenCalledWith("Show my last 20 transactions");
  });

  it("/recent defaults to 10 for invalid count", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/recent", "abc", deps);

    expect(deps.sendMessage).toHaveBeenCalledWith("Show my last 10 transactions");
  });

  it("/undo calls undo command and inserts success system message", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/undo", "", deps);

    expect(deps.undoLastTransaction).toHaveBeenCalled();
    expect(deps.addSystemMessage).toHaveBeenCalledWith("Last transaction undone.", "info");
  });

  it("/undo inserts error system message when command fails", async () => {
    const deps = makeDeps({
      undoLastTransaction: vi.fn().mockRejectedValue(new Error("nope")),
    });
    await dispatchSlashCommand("/undo", "", deps);

    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      "Nothing to undo, or the last transaction cannot be reversed.",
      "error",
    );
  });

  it("/help inserts the commands artifact locally", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/help", "", deps);

    expect(deps.addArtifactMessage).toHaveBeenCalledWith(
      "Commands",
      expect.stringContaining("/budget"),
    );
  });

  it("/defaults inserts AI defaults artifact", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/defaults", "", deps);

    expect(deps.getAIDefaults).toHaveBeenCalled();
    expect(deps.addArtifactMessage).toHaveBeenCalledWith(
      "AI Defaults",
      expect.stringContaining("timezone: America/Chicago"),
    );
  });

  it("unknown command inserts standard error system message", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/notacommand", "", deps);

    expect(deps.addSystemMessage).toHaveBeenCalledWith(UNKNOWN_COMMAND_MESSAGE, "error");
  });
});

describe("useSlashDispatch", () => {
  beforeEach(() => {
    useChatStore.setState({ localMessages: [] });
    vi.mocked(invoke).mockReset();
  });

  it("routes /budget through sendMessage path", async () => {
    // The real useSendMessage invokes submit_message — return a valid text response
    // so the async tail of sendMessage resolves cleanly.
    vi.mocked(invoke).mockResolvedValue({ kind: "text", text: "ok" });
    const { result } = renderHook(() => useSlashDispatch());

    await act(async () => {
      await result.current("/budget");
    });

    const message = useChatStore.getState().localMessages[0];
    expect(message).toMatchObject({
      kind: "user",
      text: "Show envelope budget status for the current month",
    });
  });

  it("handles /undo by invoking command and adding system message", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { result } = renderHook(() => useSlashDispatch());

    await act(async () => {
      await result.current("/undo");
    });

    expect(invoke).toHaveBeenCalledWith("undo_last_transaction");
    const message = useChatStore.getState().localMessages[0];
    expect(message).toMatchObject({ kind: "system", text: "Last transaction undone." });
  });

  it("handles /defaults by loading defaults and inserting an artifact", async () => {
    vi.mocked(invoke).mockResolvedValue({ timezone: "America/Chicago" });
    const { result } = renderHook(() => useSlashDispatch());

    await act(async () => {
      await result.current("/defaults");
    });

    expect(invoke).toHaveBeenCalledWith("get_ai_defaults");
    const artifact = useChatStore.getState().localMessages[0];
    expect(artifact).toMatchObject({
      kind: "artifact",
      title: "AI Defaults",
      content: expect.stringContaining("timezone: America/Chicago"),
    });
  });
});
