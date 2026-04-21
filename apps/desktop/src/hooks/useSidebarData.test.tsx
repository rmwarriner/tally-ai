import "@testing-library/jest-dom/vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  useAccountBalances,
  useEnvelopeStatuses,
  usePendingTransactions,
} from "./useSidebarData";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("useSidebarData", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("fetches account balances", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const { result } = renderHook(() => useAccountBalances(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockInvoke).toHaveBeenCalledWith("get_account_balances");
  });

  it("fetches envelope statuses", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const { result } = renderHook(() => useEnvelopeStatuses(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockInvoke).toHaveBeenCalledWith("get_current_envelope_periods");
  });

  it("fetches pending transactions", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const { result } = renderHook(() => usePendingTransactions(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockInvoke).toHaveBeenCalledWith("get_pending_transactions");
  });
});
