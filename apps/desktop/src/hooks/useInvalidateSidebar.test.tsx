import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import { useInvalidateSidebar } from "./useInvalidateSidebar";

describe("useInvalidateSidebar", () => {
  it("invalidates queries under the 'sidebar' root key", async () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    const { result } = renderHook(() => useInvalidateSidebar(), { wrapper });

    await act(async () => {
      await result.current();
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["sidebar"] });
  });
});
