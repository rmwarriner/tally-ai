import { beforeEach, describe, expect, it } from "vitest";

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
