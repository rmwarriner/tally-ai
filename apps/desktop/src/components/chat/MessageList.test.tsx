import "@testing-library/jest-dom/vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ReactNode } from "react";

import { MessageList } from "./MessageList";
import type { ChatMessage } from "./chatTypes";

function makeWrapper() {
  const queryClient = new QueryClient();
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return Wrapper;
}

function buildMessages(): ChatMessage[] {
  const now = Date.now();
  const yesterday = now - 86_400_000;

  return [
    { kind: "user", id: "1", ts: yesterday, text: "Old" },
    { kind: "ai", id: "2", ts: yesterday, text: "Old reply" },
    { kind: "proactive", id: "3", ts: now, text: "Heads up" },
    {
      kind: "transaction",
      id: "4",
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
    { kind: "artifact", id: "5", ts: now, artifact_id: "art_1", title: "Balance report" },
    { kind: "user", id: "6", ts: now, text: "New" },
  ];
}

describe("MessageList", () => {
  it("renders all message kinds", () => {
    render(<MessageList messages={buildMessages()} />, { wrapper: makeWrapper() });

    expect(screen.getByText("Old")).toBeInTheDocument();
    expect(screen.getByText("Old reply")).toBeInTheDocument();
    expect(screen.getByText("Heads up")).toBeInTheDocument();
    expect(screen.getByRole("article", { name: /transaction: trader joe's, \$42.99/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/artifact card placeholder/i)).toBeInTheDocument();
  });

  it("shows date separators", () => {
    render(<MessageList messages={buildMessages()} />, { wrapper: makeWrapper() });

    expect(screen.getByRole("separator", { name: /today/i })).toBeInTheDocument();
  });

  it("shows avatar for ai and proactive messages", () => {
    render(<MessageList messages={buildMessages()} />, { wrapper: makeWrapper() });

    expect(screen.getByLabelText(/ai avatar/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/proactive avatar/i)).toBeInTheDocument();
  });
});
