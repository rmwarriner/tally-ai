import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock Tauri so the onboarding engine doesn't make real IPC calls in tests.
// The mock dispatches by command name so each test gets a shape the caller expects.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "submit_message") {
      return { kind: "text", text: "ok" };
    }
    if (cmd === "list_chat_messages") {
      return [];
    }
    return true;
  }),
}));

import App from "./App";
import { useChatStore } from "./stores/chatStore";
import { useOnboardingStore, getOnboardingInitialState } from "./stores/onboardingStore";

describe("App", () => {
  beforeEach(() => {
    useChatStore.setState({ localMessages: [] });
    // Bypass onboarding so input routing behaves normally
    useOnboardingStore.setState({ ...getOnboardingInitialState(), phase: "complete" });
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
  });

  it("renders app shell with sidebar and chat regions", () => {
    render(<App />);

    expect(screen.getByRole("complementary", { name: /financial health/i })).toBeInTheDocument();
    expect(screen.getByRole("log", { name: /chat thread/i })).toBeInTheDocument();
  });

  it("Cmd/Ctrl+B cycles sidebar width open -> icon -> hidden -> open", () => {
    render(<App />);

    const sidebar = screen.getByRole("complementary", { name: /financial health/i });
    expect(sidebar).toHaveStyle({ width: "280px" });

    fireEvent.keyDown(window, { key: "b", metaKey: true });
    expect(sidebar).toHaveStyle({ width: "48px" });

    fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    expect(sidebar).toHaveStyle({ width: "0px" });

    fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    expect(sidebar).toHaveStyle({ width: "280px" });
  });

  it("routes plain text through sendMessage", () => {
    render(<App />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "Hello" } });
    fireEvent.keyDown(textbox, { key: "Enter" });

    expect(useChatStore.getState().localMessages[0]).toMatchObject({
      kind: "user",
      text: "Hello",
    });
  });

  it("routes slash commands through slash dispatch without user echo", () => {
    render(<App />);

    const textbox = screen.getByRole("textbox", { name: /chat input/i });
    fireEvent.change(textbox, { target: { value: "/help " } });
    fireEvent.keyDown(textbox, { key: "Enter" });

    expect(useChatStore.getState().localMessages[0]).toMatchObject({
      kind: "artifact",
      title: "Commands",
    });
  });
});
