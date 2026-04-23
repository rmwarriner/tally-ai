import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ChatThread } from "./ChatThread";
import type { ChatMessage } from "./chatTypes";
import { useChatHistory } from "../../hooks/useChatHistory";

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
