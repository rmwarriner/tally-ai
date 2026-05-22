import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement, ReactNode } from "react";

/// Shared TanStack Query test wrapper. Replaces the per-file inline
/// `new QueryClient()` + `QueryClientProvider` boilerplate.
///
/// Returns both the client and a wrapper component so tests that need to
/// poke at the client directly (e.g. to assert `invalidateQueries` calls)
/// still can. Tests that only need the wrapper can call
/// `makeQueryWrapper().wrapper`.
///
/// Defaults: retries disabled (tests shouldn't wait on flaky retries),
/// gcTime/staleTime zeroed (avoid bleed between tests). Override via the
/// `defaultOptions` argument if a test needs different behavior.
export function makeQueryWrapper(
  defaultOptions: ConstructorParameters<typeof QueryClient>[0] = {},
): {
  queryClient: QueryClient;
  wrapper: (props: { children: ReactNode }) => ReactElement;
} {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
      ...(defaultOptions.defaultOptions ?? {}),
    },
    ...defaultOptions,
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, wrapper };
}
