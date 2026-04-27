import "@testing-library/jest-dom/vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { checkA11y, expectNoA11yViolations } from "../../test/axe";
import { ChatThread } from "./ChatThread";
import type { ChatMessage } from "./chatTypes";
import { useChatHistory } from "../../hooks/useChatHistory";
import { useChatStore } from "../../stores/chatStore";

function makeWrapper() {
  const queryClient = new QueryClient();
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

vi.mock("../../hooks/useChatHistory", () => ({
  useChatHistory: vi.fn(),
}));

const mockUseChatHistory = vi.mocked(useChatHistory);

function makeMessage(id: string, ts: number, text: string): ChatMessage {
  return { kind: "user", id, ts, text };
}

describe("ChatThread", () => {
  beforeEach(() => {
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
    mockUseChatHistory.mockReset();
    useChatStore.setState({ localMessages: [] });
  });

  it("renders user and ai messages", () => {
    const now = Date.now();
    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "Hello"), { kind: "ai", id: "2", ts: now, text: "Hi there" }]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    render(<ChatThread />);

    expect(screen.getByText("Hello")).toBeInTheDocument();
    expect(screen.getByText("Hi there")).toBeInTheDocument();
  });

  it("calls fetchNextPage when loading earlier messages", () => {
    const now = Date.now();
    const fetchNextPage = vi.fn();

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "Hello")]],
        pageParams: [undefined],
      },
      hasNextPage: true,
      isFetchingNextPage: false,
      fetchNextPage,
    } as unknown as ReturnType<typeof useChatHistory>);

    render(<ChatThread />);

    fireEvent.click(screen.getByRole("button", { name: /load earlier messages/i }));
    expect(fetchNextPage).toHaveBeenCalled();
  });

  it("shows new message pill when user is scrolled up and new message arrives", () => {
    const now = Date.now();

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    const { rerender } = render(<ChatThread />);
    const thread = screen.getByRole("log", { name: /chat thread/i });

    Object.defineProperty(thread, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(thread, "clientHeight", { configurable: true, value: 200 });
    Object.defineProperty(thread, "scrollTop", { configurable: true, writable: true, value: 0 });
    fireEvent.scroll(thread);

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First"), makeMessage("2", now + 1000, "Second")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    rerender(<ChatThread />);

    expect(screen.getByRole("button", { name: /new message/i })).toBeInTheDocument();
  });

  it("auto-scrolls when a new message arrives and user is near bottom", () => {
    const now = Date.now();
    const scrollIntoView = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoView;

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    const { rerender } = render(<ChatThread />);
    const thread = screen.getByRole("log", { name: /chat thread/i });

    Object.defineProperty(thread, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(thread, "clientHeight", { configurable: true, value: 200 });
    Object.defineProperty(thread, "scrollTop", { configurable: true, writable: true, value: 740 });
    fireEvent.scroll(thread);

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First"), makeMessage("2", now + 1000, "Second")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    rerender(<ChatThread />);

    expect(scrollIntoView).toHaveBeenLastCalledWith({ behavior: "smooth" });
    expect(screen.queryByRole("button", { name: /new message/i })).not.toBeInTheDocument();
  });

  it("jumps to bottom and hides the new message pill when clicked", () => {
    const now = Date.now();
    const scrollIntoView = vi.fn();
    window.HTMLElement.prototype.scrollIntoView = scrollIntoView;

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    const { rerender } = render(<ChatThread />);
    const thread = screen.getByRole("log", { name: /chat thread/i });

    Object.defineProperty(thread, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(thread, "clientHeight", { configurable: true, value: 200 });
    Object.defineProperty(thread, "scrollTop", { configurable: true, writable: true, value: 0 });
    fireEvent.scroll(thread);

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First"), makeMessage("2", now + 1000, "Second")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    rerender(<ChatThread />);

    const pill = screen.getByRole("button", { name: /new message/i });
    fireEvent.click(pill);

    expect(scrollIntoView).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /new message/i })).not.toBeInTheDocument();
  });

  it("hides the new message pill when user scrolls back near the bottom", () => {
    const now = Date.now();

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    const { rerender } = render(<ChatThread />);
    const thread = screen.getByRole("log", { name: /chat thread/i });

    Object.defineProperty(thread, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(thread, "clientHeight", { configurable: true, value: 200 });
    Object.defineProperty(thread, "scrollTop", { configurable: true, writable: true, value: 0 });
    fireEvent.scroll(thread);

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now, "First"), makeMessage("2", now + 1000, "Second")]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    rerender(<ChatThread />);
    expect(screen.getByRole("button", { name: /new message/i })).toBeInTheDocument();

    Object.defineProperty(thread, "scrollTop", { configurable: true, writable: true, value: 740 });
    fireEvent.scroll(thread);

    expect(screen.queryByRole("button", { name: /new message/i })).not.toBeInTheDocument();
  });

  it("renders a date separator between messages from different local days", () => {
    const today = new Date("2026-04-23T12:00:00.000Z").getTime();
    const yesterday = today - 86_400_000;

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[
          makeMessage("1", yesterday, "Old"),
          makeMessage("2", today, "New"),
        ]],
        pageParams: [undefined],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    render(<ChatThread />);

    // Two unique date keys -> two separators.
    expect(screen.getAllByRole("separator").length).toBeGreaterThanOrEqual(2);
  });

  it("renders messages of every kind in the union without crashing", () => {
    const now = Date.now();
    const messages: ChatMessage[] = [
      { kind: "user", id: "u1", ts: now, text: "User msg" },
      { kind: "ai", id: "a1", ts: now, text: "AI msg" },
      { kind: "proactive", id: "p1", ts: now, text: "Proactive msg" },
      { kind: "system", id: "s1", ts: now, text: "System msg", tone: "info" },
      {
        kind: "transaction",
        id: "t1",
        ts: now,
        transaction_id: "txn_1",
        state: "posted",
        transaction: {
          id: "txn_1",
          payee: "Trader Joe's",
          txn_date: now,
          amount_cents: 4299,
          account_name: "Checking",
          lines: [],
        },
      },
      { kind: "artifact", id: "ar1", ts: now, artifact_id: "art_1", title: "ArtifactTitle" },
      {
        kind: "setup_card",
        id: "sc1",
        ts: now,
        variant: "household_created",
        title: "HouseholdSetup",
        detail: "Created",
      },
      {
        kind: "handoff",
        id: "h1",
        ts: now,
        householdName: "SmithHousehold",
        accountCount: 2,
        envelopeCount: 3,
        starterPrompts: ["StarterPrompt"],
      },
    ];

    mockUseChatHistory.mockReturnValue({
      data: { pages: [messages], pageParams: [undefined] },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    render(<ChatThread />, { wrapper: makeWrapper() });

    expect(screen.getByText("User msg")).toBeInTheDocument();
    expect(screen.getByText("AI msg")).toBeInTheDocument();
    expect(screen.getByText("Proactive msg")).toBeInTheDocument();
    expect(screen.getByText("System msg")).toBeInTheDocument();
    expect(screen.getByText("Trader Joe's")).toBeInTheDocument();
    expect(screen.getByText("ArtifactTitle")).toBeInTheDocument();
    expect(screen.getByText("HouseholdSetup")).toBeInTheDocument();
    expect(screen.getByText(/SmithHousehold/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /StarterPrompt/i })).toBeInTheDocument();
  });

  it("passes axe with messages rendered", async () => {
    const now = Date.now();
    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[
          makeMessage("1", now, "Hello"),
          { kind: "ai", id: "2", ts: now, text: "Hi" } as ChatMessage,
        ]],
        pageParams: [undefined],
      },
      hasNextPage: true,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    const { container } = render(<ChatThread />);
    expectNoA11yViolations(await checkA11y(container));
  });

  it("passes axe with empty placeholder", async () => {
    mockUseChatHistory.mockReturnValue({
      data: { pages: [[]], pageParams: [undefined] },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage: vi.fn(),
    } as unknown as ReturnType<typeof useChatHistory>);

    const { container } = render(<ChatThread />);
    expectNoA11yViolations(await checkA11y(container));
  });

  it("preserves scroll position after loading earlier messages", () => {
    const now = Date.now();
    const fetchNextPage = vi.fn();

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("2", now, "Current")]],
        pageParams: [undefined],
      },
      hasNextPage: true,
      isFetchingNextPage: false,
      fetchNextPage,
    } as unknown as ReturnType<typeof useChatHistory>);

    const { rerender } = render(<ChatThread />);
    const thread = screen.getByRole("log", { name: /chat thread/i });

    Object.defineProperty(thread, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(thread, "clientHeight", { configurable: true, value: 200 });
    Object.defineProperty(thread, "scrollTop", { configurable: true, writable: true, value: 300 });

    fireEvent.click(screen.getByRole("button", { name: /load earlier messages/i }));

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("2", now, "Current")]],
        pageParams: [undefined, "1"],
      },
      hasNextPage: true,
      isFetchingNextPage: true,
      fetchNextPage,
    } as unknown as ReturnType<typeof useChatHistory>);
    rerender(<ChatThread />);

    Object.defineProperty(thread, "scrollHeight", { configurable: true, value: 1300 });

    mockUseChatHistory.mockReturnValue({
      data: {
        pages: [[makeMessage("1", now - 1000, "Older"), makeMessage("2", now, "Current")]],
        pageParams: [undefined, "1"],
      },
      hasNextPage: false,
      isFetchingNextPage: false,
      fetchNextPage,
    } as unknown as ReturnType<typeof useChatHistory>);
    rerender(<ChatThread />);

    expect(thread.scrollTop).toBe(600);
  });
});
