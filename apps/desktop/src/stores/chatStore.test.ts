import { beforeEach, describe, expect, it } from "vitest";
import type { RecoveryError } from "@tally/core-types";

import { useChatStore } from "./chatStore";

describe("useChatStore", () => {
  beforeEach(() => {
    useChatStore.setState({ localMessages: [] });
  });

  it("adds an explicit local message", () => {
    useChatStore.getState().addLocalMessage({
      kind: "system",
      id: "msg_1",
      ts: Date.now(),
      text: "Hello",
      tone: "info",
    });

    expect(useChatStore.getState().localMessages).toHaveLength(1);
    expect(useChatStore.getState().localMessages[0]?.kind).toBe("system");
  });

  it("adds a user message", () => {
    useChatStore.getState().addUserMessage("hi there");

    const message = useChatStore.getState().localMessages[0];
    expect(message).toMatchObject({ kind: "user", text: "hi there" });
  });

  it("adds a system message with tone", () => {
    useChatStore.getState().addSystemMessage("Oops", "error");

    const message = useChatStore.getState().localMessages[0];
    expect(message).toMatchObject({ kind: "system", text: "Oops", tone: "error" });
  });

  it("adds an artifact message with title and content", () => {
    useChatStore.getState().addArtifactMessage("Commands", "/help");

    const message = useChatStore.getState().localMessages[0];
    expect(message).toMatchObject({
      kind: "artifact",
      title: "Commands",
      content: "/help",
    });
  });
});

describe("appendAdvisory", () => {
  beforeEach(() => {
    useChatStore.setState({ localMessages: [] });
  });

  it("appends a proactive chat message reflecting the RecoveryError", () => {
    const err: RecoveryError = {
      message: "Boom",
      recovery: [{ kind: "SHOW_HELP", label: "Get help", is_primary: true }],
    };

    useChatStore.getState().appendAdvisory!(err);

    const messages = useChatStore.getState().localMessages;
    expect(messages).toHaveLength(1);
    const last = messages[messages.length - 1]!;
    expect(last.kind).toBe("proactive");
    if (last.kind === "proactive") {
      expect(last.text).toBe("Boom");
      expect(last.recovery).toEqual([
        { kind: "SHOW_HELP", label: "Get help", is_primary: true },
      ]);
      expect(typeof last.id).toBe("string");
      expect(typeof last.ts).toBe("number");
    }
  });

  it("preserves every recovery action on the appended message", () => {
    const err: RecoveryError = {
      message: "Bang",
      recovery: [
        { kind: "EDIT_FIELD", label: "Edit", is_primary: true },
        { kind: "DISCARD", label: "Discard", is_primary: false },
      ],
    };

    useChatStore.getState().appendAdvisory!(err);

    const messages = useChatStore.getState().localMessages;
    const last = messages[messages.length - 1]!;
    expect(last.kind).toBe("proactive");
    if (last.kind === "proactive") {
      expect(last.recovery).toHaveLength(2);
      const kinds = last.recovery?.map((a) => a.kind);
      expect(kinds).toEqual(["EDIT_FIELD", "DISCARD"]);
      const primary = last.recovery?.find((a) => a.is_primary);
      expect(primary?.kind).toBe("EDIT_FIELD");
    }
  });

  it("snapshots recovery so later mutation of the input does not bleed in", () => {
    const recovery = [
      { kind: "SHOW_HELP" as const, label: "Help", is_primary: true },
    ];
    const err: RecoveryError = {
      message: "Snapshot",
      recovery: [recovery[0]!],
    };

    useChatStore.getState().appendAdvisory!(err);
    // Mutate the original after the call.
    err.recovery.push({ kind: "DISCARD", label: "Discard", is_primary: false });

    const last = useChatStore.getState().localMessages.at(-1)!;
    expect(last.kind).toBe("proactive");
    if (last.kind === "proactive") {
      expect(last.recovery).toHaveLength(1);
    }
  });
});
