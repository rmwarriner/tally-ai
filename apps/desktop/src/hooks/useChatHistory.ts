import { useInfiniteQuery } from "@tanstack/react-query";

import type { ChatMessage } from "../components/chat/chatTypes";
import { safeInvoke } from "../lib/safeInvoke";

const PAGE_SIZE = 30;

function getOldestMessageId(messages: ChatMessage[]): string | undefined {
  if (messages.length === 0) {
    return undefined;
  }

  return [...messages].sort((a, b) => a.ts - b.ts)[0]?.id;
}

export function useChatHistory() {
  return useInfiniteQuery({
    queryKey: ["chat", "history"],
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam }) => {
      const r = await safeInvoke<ChatMessage[]>("get_chat_history", {
        before: pageParam,
        limit: PAGE_SIZE,
      });
      if (!r.ok) throw r.error;
      return r.value;
    },
    getNextPageParam: (lastPage) => {
      if (lastPage.length < PAGE_SIZE) {
        return undefined;
      }
      return getOldestMessageId(lastPage);
    },
    staleTime: 10_000,
  });
}
