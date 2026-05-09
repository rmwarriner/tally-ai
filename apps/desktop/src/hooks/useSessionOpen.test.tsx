import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore, type ProactiveInsight } from "../stores/chatStore";
import { useOnboardingStore } from "../stores/onboardingStore";
import { useSessionOpen } from "./useSessionOpen";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const briefing: ProactiveInsight = {
  id: "01BRIEFINGAAAAAAAAAAAAAAAA",
  kind: "morning_briefing",
  category: "briefing",
  user_message: "Good morning! Cash on hand: $1,500.00.",
  created_at: 1_716_000_000_000,
};

beforeEach(() => {
  useChatStore.setState({ localMessages: [] });
  vi.mocked(invoke).mockReset();
});

afterEach(() => {
  useOnboardingStore.setState({ phase: "checking" });
});

describe("useSessionOpen", () => {
  it("does not call session_open while onboarding is incomplete", async () => {
    useOnboardingStore.setState({ phase: "fresh_start" });
    renderHook(() => useSessionOpen());

    // Give any unintended effect a tick to run.
    await new Promise((r) => setTimeout(r, 0));
    expect(invoke).not.toHaveBeenCalled();
  });

  it("appends the briefing as a proactive message when session_open returns one", async () => {
    useOnboardingStore.setState({ phase: "complete" });
    vi.mocked(invoke).mockResolvedValue(briefing);

    renderHook(() => useSessionOpen());

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("session_open", undefined);
    });
    await waitFor(() => {
      const msg = useChatStore.getState().localMessages[0];
      expect(msg).toMatchObject({
        kind: "proactive",
        text: briefing.user_message,
        category: "briefing",
      });
    });
  });

  it("appends nothing when session_open resolves to null (gated/dedup)", async () => {
    useOnboardingStore.setState({ phase: "complete" });
    vi.mocked(invoke).mockResolvedValue(null);

    renderHook(() => useSessionOpen());

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("session_open", undefined);
    });
    expect(useChatStore.getState().localMessages).toHaveLength(0);
  });

  it("only fires once per mount even if React re-renders", async () => {
    useOnboardingStore.setState({ phase: "complete" });
    vi.mocked(invoke).mockResolvedValue(null);

    const { rerender } = renderHook(() => useSessionOpen());
    rerender();
    rerender();

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledTimes(1);
    });
  });
});
