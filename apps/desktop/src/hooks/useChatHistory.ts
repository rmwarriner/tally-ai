import { useInfiniteQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import type { ChatMessage } from "../components/chat/chatTypes";

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
    queryFn: async ({ pageParam }) =>
      invoke<ChatMessage[]>("get_chat_history", {
        before: pageParam,
        limit: PAGE_SIZE,
      }),
    getNextPageParam: (lastPage) => {
      if (lastPage.length < PAGE_SIZE) {
        return undefined;
      }
      return getOldestMessageId(lastPage);
    },
    staleTime: 10_000,
  });
}
