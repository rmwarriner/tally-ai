import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

import type { ChatMessage } from "../components/chat/chatTypes";
import { useChatStore } from "../stores/chatStore";
import { useOnboardingStore } from "../stores/onboardingStore";

interface ChatMessageRow {
  id: string;
  kind: string;
  payload: string;
  ts: number;
}

export interface ChatPersistenceDeps {
  invoke: typeof tauriInvoke;
}

async function loadHistory(invoke: typeof tauriInvoke): Promise<ChatMessage[]> {
  const rows = await invoke<ChatMessageRow[]>("list_chat_messages", {
    args: { before_ts: null, limit: 500 },
  });
  const messages: ChatMessage[] = [];
  for (const row of rows) {
    try {
      messages.push(JSON.parse(row.payload) as ChatMessage);
    } catch {
      // Payload corruption is non-fatal — skip the row rather than breaking the thread.
    }
  }
  return messages.reverse();
}

export function buildChatPersistence(deps: ChatPersistenceDeps) {
  async function hydrate(): Promise<void> {
    const currentLength = useChatStore.getState().localMessages.length;
    if (currentLength > 0) return;
    try {
      const messages = await loadHistory(deps.invoke);
      useChatStore.setState({ localMessages: messages });
    } catch (err) {
      console.warn("chat hydrate failed:", err);
    }
  }

  function persist(message: ChatMessage): Promise<void> {
    return deps
      .invoke<void>("append_chat_message", {
        args: {
          id: message.id,
          kind: message.kind,
          payload: JSON.stringify(message),
          ts: message.ts,
        },
      })
      .catch((err) => {
        console.warn("chat persist failed:", message.id, err);
      });
  }

  return { hydrate, persist };
}

export function useChatPersistence(deps: ChatPersistenceDeps = { invoke: tauriInvoke }): void {
  const hydrateDoneRef = useRef(false);
  const persistedIdsRef = useRef<Set<string>>(new Set());
  const phase = useOnboardingStore((s) => s.phase);

  useEffect(() => {
    if (phase !== "complete" || hydrateDoneRef.current) return;
    hydrateDoneRef.current = true;

    const { hydrate } = buildChatPersistence(deps);
    const existing = useChatStore.getState().localMessages;

    if (existing.length === 0) {
      void hydrate().then(() => {
        for (const m of useChatStore.getState().localMessages) {
          persistedIdsRef.current.add(m.id);
        }
      });
    } else {
      // Fresh-user path: onboarding messages are ephemeral. Mark them as
      // already-persisted so the subscriber below doesn't back-write them.
      for (const m of existing) persistedIdsRef.current.add(m.id);
    }
  }, [phase, deps]);

  useEffect(() => {
    const { persist } = buildChatPersistence(deps);
    return useChatStore.subscribe((state) => {
      if (!hydrateDoneRef.current) return;
      for (const m of state.localMessages) {
        if (persistedIdsRef.current.has(m.id)) continue;
        persistedIdsRef.current.add(m.id);
        void persist(m);
      }
    });
  }, [deps]);
}
