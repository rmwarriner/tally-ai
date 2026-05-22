import "@testing-library/jest-dom/vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { makeQueryWrapper } from "../test/queryWrapper";
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
  return makeQueryWrapper().wrapper;
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

    expect(mockInvoke).toHaveBeenCalledWith("get_account_balances", undefined);
  });

  it("fetches envelope statuses", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const { result } = renderHook(() => useEnvelopeStatuses(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockInvoke).toHaveBeenCalledWith("get_current_envelope_periods", undefined);
  });

  it("fetches pending transactions", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    const { result } = renderHook(() => usePendingTransactions(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isSuccess).toBe(true);
    });

    expect(mockInvoke).toHaveBeenCalledWith("get_pending_transactions", undefined);
  });
});
