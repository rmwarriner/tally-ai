import { create } from "zustand";

import type { ChatMessage } from "../components/chat/chatTypes";
import { generateUlid } from "../utils/ulid";

interface ChatStore {
  localMessages: ChatMessage[];
  addLocalMessage: (message: ChatMessage) => void;
  addUserMessage: (text: string) => void;
  addSystemMessage: (text: string, tone?: "info" | "error") => void;
  addArtifactMessage: (title: string, content: string) => void;
}

function makeBaseMessage<K extends ChatMessage["kind"]>(kind: K): { kind: K; id: string; ts: number } {
  return {
    kind,
    id: generateUlid(),
    ts: Date.now(),
  };
}

export const useChatStore = create<ChatStore>((set) => ({
  localMessages: [],
  addLocalMessage: (message) => {
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addUserMessage: (text) => {
    const message: ChatMessage = {
      ...makeBaseMessage("user"),
      text,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addSystemMessage: (text, tone = "info") => {
    const message: ChatMessage = {
      ...makeBaseMessage("system"),
      text,
      tone,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addArtifactMessage: (title, content) => {
    const id = generateUlid();
    const message: ChatMessage = {
      ...makeBaseMessage("artifact"),
      artifact_id: id,
      title,
      content,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
}));
