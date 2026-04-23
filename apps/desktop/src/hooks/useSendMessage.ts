import { useCallback } from "react";

import { useChatStore } from "../stores/chatStore";

export function useSendMessage() {
  const addUserMessage = useChatStore((state) => state.addUserMessage);

  return useCallback(
    (text: string) => {
      addUserMessage(text);
    },
    [addUserMessage],
  );
}
