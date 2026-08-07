import { create } from "zustand";

import type { ChatMessage } from "../components/chat/chatTypes";
import type { SetupCardVariant } from "../components/onboarding/SetupCard";
import type {
  ImportPlan,
  QifBalanceReportArtifact,
  QifImportPlan,
  RecoveryError,
} from "@tally/core-types";
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
  addQifMappingMessage: (plan: QifImportPlan, skippedSecurityTrades: number) => void;
  addQifReconcileMessage: (report: QifBalanceReportArtifact) => void;
  updateMessage: (id: string, patch: Partial<ChatMessage>) => void;
  removeMessage: (id: string) => void;
  // Task 12: convert a RecoveryError into a proactive-advisory chat message
  // and append it. Kept optional on the interface so existing call sites
  // (e.g. safeInvoke's defaultDispatch) keep type-checking via optional
  // chaining; the implementation below always supplies it.
  appendAdvisory?: (err: RecoveryError) => void;
  /// Wire-shape from `core::insight::ProactiveInsight`. Backs the proactive
  /// engine producers (envelope alerts, possible duplicate, morning briefing).
  appendInsight: (insight: ProactiveInsight) => void;
}

/// Mirrors `core::insight::ProactiveInsight` (snake_case JSON via serde).
export interface ProactiveInsight {
  id: string;
  kind:
    | "envelope_over"
    | "envelope_approaching"
    | "possible_duplicate"
    | "morning_briefing";
  category: "alert" | "insight" | "briefing";
  user_message: string;
  created_at: number;
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
  addQifMappingMessage: (plan, skippedSecurityTrades) => {
    const message: ChatMessage = {
      ...makeBaseMessage("qif_mapping"),
      plan,
      skippedSecurityTrades,
    };
    set((state) => ({ localMessages: [...state.localMessages, message] }));
  },
  addQifReconcileMessage: (report) => {
    const message: ChatMessage = {
      ...makeBaseMessage("qif_reconcile"),
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
  appendInsight: (insight) => {
    const message: ChatMessage = {
      // Reuse the backend ULID so the persistence layer can dedup if a
      // single insight ever reaches us twice within a session.
      kind: "proactive",
      id: insight.id,
      ts: insight.created_at,
      text: insight.user_message,
      category: insight.category,
    };
    set((state) =>
      state.localMessages.some((m) => m.id === insight.id)
        ? state
        : { localMessages: [...state.localMessages, message] },
    );
  },
}));
