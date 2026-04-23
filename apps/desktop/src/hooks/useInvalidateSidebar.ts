import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

export function useInvalidateSidebar() {
  const queryClient = useQueryClient();
  return useCallback(
    () => queryClient.invalidateQueries({ queryKey: ["sidebar"] }),
    [queryClient],
  );
}
