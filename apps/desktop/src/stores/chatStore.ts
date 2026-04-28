import { create } from "zustand";

import type { ChatMessage } from "../components/chat/chatTypes";
import type { SetupCardVariant } from "../components/onboarding/SetupCard";
import type { ImportPlan, RecoveryError } from "@tally/core-types";
import type { GnuCashReconcileReport } from "../components/artifacts/GnuCashReconcileCard";
import { generateUlid } from "../utils/ulid";

interface ChatStore {
  localMessages: ChatMessage[];
  addLocalMessage: (message: ChatMessage) => void;
  addUserMessage: (text: string) => void;
  addSystemMessage: (text: string, tone?: "info" | "error") => void;
  addArtifactMessage: (title: string, content: string) => void;
  addSetupCard: (variant: SetupCardVariant, title: string, detail: string) => void;
  addHandoffMessage: (
    householdName: string,
    accountCount: number,
    envelopeCount: number,
    starterPrompts: string[],
  ) => void;
  addGnuCashMappingMessage: (plan: ImportPlan) => void;
  addGnuCashReconcileMessage: (report: GnuCashReconcileReport) => void;
  updateMessage: (id: string, patch: Partial<ChatMessage>) => void;
  removeMessage: (id: string) => void;
  // Task 12: convert a RecoveryError into a proactive-advisory chat message
  // and append it. Kept optional on the interface so existing call sites
  // (e.g. safeInvoke's defaultDispatch) keep type-checking via optional
  // chaining; the implementation below always supplies it.
  appendAdvisory?: (err: RecoveryError) => void;
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
  addSetupCard: (variant, title, detail) => {
    const message: ChatMessage = {
      ...makeBaseMessage("setup_card"),
      variant,
      title,
      detail,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addHandoffMessage: (householdName, accountCount, envelopeCount, starterPrompts) => {
    const message: ChatMessage = {
      ...makeBaseMessage("handoff"),
      householdName,
      accountCount,
      envelopeCount,
      starterPrompts,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addGnuCashMappingMessage: (plan) => {
    const message: ChatMessage = {
      ...makeBaseMessage("gnucash_mapping"),
      plan,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addGnuCashReconcileMessage: (report) => {
    const message: ChatMessage = {
      ...makeBaseMessage("gnucash_reconcile"),
      report,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  updateMessage: (id, patch) => {
    set((state) => ({
      localMessages: state.localMessages.map((m) =>
        m.id === id ? ({ ...m, ...patch } as ChatMessage) : m,
      ),
    }));
  },
  removeMessage: (id) => {
    set((state) => ({
      localMessages: state.localMessages.filter((m) => m.id !== id),
    }));
  },
  appendAdvisory: (err) => {
    const message: ChatMessage = {
      ...makeBaseMessage("proactive"),
      text: err.message,
      recovery: [...err.recovery],
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
}));
