import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore } from "../stores/chatStore";
import { makeQueryWrapper } from "../test/queryWrapper";
import { UNKNOWN_COMMAND_MESSAGE, dispatchSlashCommand, useSlashDispatch } from "./useSlashDispatch";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const makeWrapper = makeQueryWrapper;

function makeDeps(overrides: Partial<Parameters<typeof dispatchSlashCommand>[2]> = {}) {
  return {
    sendMessage: vi.fn(),
    addSystemMessage: vi.fn(),
    addArtifactMessage: vi.fn(),
    undoLastTransaction: vi.fn().mockResolvedValue(undefined),
    getAIDefaults: vi.fn().mockResolvedValue({ timezone: "America/Chicago", preferred_accounts: ["Checking"] }),
    setAIDefault: vi.fn().mockResolvedValue(undefined),
    invalidateSidebar: vi.fn(),
    getSensitivity: vi.fn().mockResolvedValue("normal"),
    setSensitivity: vi.fn().mockResolvedValue(undefined),
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

  it("/undo invalidates sidebar after success so balances refresh", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/undo", "", deps);

    expect(deps.invalidateSidebar).toHaveBeenCalled();
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

  it("/undo does not invalidate sidebar on failure", async () => {
    const deps = makeDeps({
      undoLastTransaction: vi.fn().mockRejectedValue(new Error("nope")),
    });
    await dispatchSlashCommand("/undo", "", deps);

    expect(deps.invalidateSidebar).not.toHaveBeenCalled();
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

  it("/defaults key=value calls setAIDefault and confirms", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/defaults", "default_currency=USD", deps);

    expect(deps.setAIDefault).toHaveBeenCalledWith("default_currency", "USD");
    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      "Set default_currency to USD.",
      "info",
    );
  });

  it("/defaults setter trims surrounding whitespace", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/defaults", "  default_currency  =  USD  ", deps);

    expect(deps.setAIDefault).toHaveBeenCalledWith("default_currency", "USD");
  });

  it("/defaults without '=' shows the usage error", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/defaults", "default_currency USD", deps);

    expect(deps.setAIDefault).not.toHaveBeenCalled();
    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("key=value"),
      "error",
    );
  });

  it("/defaults setter surfaces backend error message", async () => {
    const deps = makeDeps({
      setAIDefault: vi.fn().mockRejectedValue({ message: "Unknown AI default key 'foo'." }),
    });
    await dispatchSlashCommand("/defaults", "foo=bar", deps);

    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      "Unknown AI default key 'foo'.",
      "error",
    );
  });

  it("unknown command inserts standard error system message", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/notacommand", "", deps);

    expect(deps.addSystemMessage).toHaveBeenCalledWith(UNKNOWN_COMMAND_MESSAGE, "error");
  });

  it("/sensitivity with no args reads + reports the current value", async () => {
    const deps = makeDeps({
      getSensitivity: vi.fn().mockResolvedValue("proactive"),
    });
    await dispatchSlashCommand("/sensitivity", "", deps);

    expect(deps.getSensitivity).toHaveBeenCalled();
    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("proactive"),
      "info",
    );
  });

  it("/sensitivity quiet|normal|proactive sets the value", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/sensitivity", "quiet", deps);

    expect(deps.setSensitivity).toHaveBeenCalledWith("quiet");
    expect(deps.addSystemMessage).toHaveBeenCalledWith("Sensitivity set to quiet.", "info");
  });

  it("/sensitivity rejects invalid values with usage hint", async () => {
    const deps = makeDeps();
    await dispatchSlashCommand("/sensitivity", "loud", deps);

    expect(deps.setSensitivity).not.toHaveBeenCalled();
    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      expect.stringContaining("quiet|normal|proactive"),
      "error",
    );
  });

  it("/sensitivity surfaces an error message when set rejects", async () => {
    const deps = makeDeps({
      setSensitivity: vi.fn().mockRejectedValue(new Error("nope")),
    });
    await dispatchSlashCommand("/sensitivity", "normal", deps);

    expect(deps.addSystemMessage).toHaveBeenCalledWith(
      "Could not update sensitivity right now.",
      "error",
    );
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
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useSlashDispatch(), { wrapper });

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
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useSlashDispatch(), { wrapper });

    await act(async () => {
      await result.current("/undo");
    });

    expect(invoke).toHaveBeenCalledWith("undo_last_transaction", undefined);
    const message = useChatStore.getState().localMessages[0];
    expect(message).toMatchObject({ kind: "system", text: "Last transaction undone." });
  });

  it("invalidates sidebar after /undo so balances refresh", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { queryClient, wrapper } = makeWrapper();
    const spy = vi.spyOn(queryClient, "invalidateQueries");
    const { result } = renderHook(() => useSlashDispatch(), { wrapper });

    await act(async () => {
      await result.current("/undo");
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["sidebar"] });
  });

  it("handles /defaults by loading defaults and inserting an artifact", async () => {
    vi.mocked(invoke).mockResolvedValue({ timezone: "America/Chicago" });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useSlashDispatch(), { wrapper });

    await act(async () => {
      await result.current("/defaults");
    });

    expect(invoke).toHaveBeenCalledWith("get_ai_defaults", undefined);
    const artifact = useChatStore.getState().localMessages[0];
    expect(artifact).toMatchObject({
      kind: "artifact",
      title: "AI Defaults",
      content: expect.stringContaining("timezone: America/Chicago"),
    });
  });
});
