import { renderHook, act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { makeQueryWrapper } from "../test/queryWrapper";
import { useInvalidateSidebar } from "./useInvalidateSidebar";

describe("useInvalidateSidebar", () => {
  it("invalidates queries under the 'sidebar' root key", async () => {
    const { queryClient, wrapper } = makeQueryWrapper();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(() => useInvalidateSidebar(), { wrapper });

    await act(async () => {
      await result.current();
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["sidebar"] });
  });
});
