import { useCallback } from "react";

import { safeInvoke } from "../lib/safeInvoke";
import { useChatStore } from "../stores/chatStore";
import { useInvalidateSidebar } from "./useInvalidateSidebar";
import { useSendMessage } from "./useSendMessage";

const UNKNOWN_COMMAND_MESSAGE = "Unknown command. Type /help to see available commands.";
const HELP_ARTIFACT_CONTENT = [
  "/budget       — Show envelope budget status for the current month",
  "/balance      — Show account balances",
  "/recent       — List recent transactions (add a number: /recent 20)",
  '/fix          — Correct a transaction: /fix "groceries on Tuesday was $45"',
  "/undo         — Undo the last AI-posted transaction",
  "/help         — Show this list",
  "/defaults     — View AI entry defaults; set with /defaults key=value",
  "/sensitivity  — Set proactive engine sensitivity (quiet|normal|proactive)",
].join("\n");

type SystemTone = "info" | "error";

interface SlashDispatchDeps {
  sendMessage: (text: string) => void;
  addSystemMessage: (text: string, tone?: SystemTone) => void;
  addArtifactMessage: (title: string, content: string) => void;
  undoLastTransaction: () => Promise<void>;
  getAIDefaults: () => Promise<Record<string, unknown>>;
  setAIDefault: (key: string, value: string) => Promise<void>;
  invalidateSidebar: () => void | Promise<void>;
  getSensitivity: () => Promise<string>;
  setSensitivity: (value: string) => Promise<void>;
}

const SENSITIVITY_USAGE =
  'Usage: /sensitivity quiet|normal|proactive (or /sensitivity to view current).';

function parseRecentCount(args: string): number {
  const parsed = Number.parseInt(args.trim(), 10);
  if (Number.isNaN(parsed) || parsed <= 0) {
    return 10;
  }
  return parsed;
}

function formatDefaults(defaults: Record<string, unknown>): string {
  const entries = Object.entries(defaults);
  if (entries.length === 0) {
    return "No defaults configured yet.";
  }

  return entries
    .map(([key, value]) => {
      if (Array.isArray(value)) {
        return `${key}: ${value.join(", ")}`;
      }
      if (value && typeof value === "object") {
        return `${key}: ${JSON.stringify(value)}`;
      }
      return `${key}: ${String(value)}`;
    })
    .join("\n");
}

export async function dispatchSlashCommand(
  command: string,
  args: string,
  deps: SlashDispatchDeps,
): Promise<void> {
  switch (command) {
    case "/budget":
      deps.sendMessage("Show envelope budget status for the current month");
      return;
    case "/balance":
      deps.sendMessage("Show all account balances");
      return;
    case "/recent":
      deps.sendMessage(`Show my last ${parseRecentCount(args)} transactions`);
      return;
    case "/fix":
      deps.sendMessage(`Fix: ${args}`.trimEnd());
      return;
    case "/undo":
      try {
        await deps.undoLastTransaction();
        deps.addSystemMessage("Last transaction undone.", "info");
        void deps.invalidateSidebar();
      } catch {
        deps.addSystemMessage(
          "Nothing to undo, or the last transaction cannot be reversed.",
          "error",
        );
      }
      return;
    case "/help":
      deps.addArtifactMessage("Commands", HELP_ARTIFACT_CONTENT);
      return;
    case "/defaults":
      await handleDefaults(args, deps);
      return;
    case "/sensitivity":
      await handleSensitivity(args, deps);
      return;
    default:
      deps.addSystemMessage(UNKNOWN_COMMAND_MESSAGE, "error");
  }
}

function parseRawSlash(raw: string): { command: string; args: string } {
  const trimmed = raw.trim();
  const [command = "", ...argParts] = trimmed.split(/\s+/);
  return { command, args: argParts.join(" ") };
}

async function handleDefaults(args: string, deps: SlashDispatchDeps): Promise<void> {
  const raw = args.trim();
  if (raw.length === 0) {
    try {
      const defaults = await deps.getAIDefaults();
      deps.addArtifactMessage("AI Defaults", formatDefaults(defaults));
    } catch {
      deps.addSystemMessage("Could not load AI defaults right now.", "error");
    }
    return;
  }
  // Setter form: `key=value`. Value may contain `=`, so split on the first.
  const eq = raw.indexOf("=");
  if (eq === -1) {
    deps.addSystemMessage(
      "Usage: /defaults (to view) or /defaults key=value (to set).",
      "error",
    );
    return;
  }
  const key = raw.slice(0, eq).trim();
  const value = raw.slice(eq + 1).trim();
  if (key.length === 0 || value.length === 0) {
    deps.addSystemMessage(
      "Usage: /defaults key=value with non-empty key and value.",
      "error",
    );
    return;
  }
  try {
    await deps.setAIDefault(key, value);
    deps.addSystemMessage(`Set ${key} to ${value}.`, "info");
  } catch (err) {
    const message =
      err && typeof err === "object" && "message" in err && typeof err.message === "string"
        ? err.message
        : "Could not update that default.";
    deps.addSystemMessage(message, "error");
  }
}

const SENSITIVITY_VALUES = new Set(["quiet", "normal", "proactive"]);

async function handleSensitivity(args: string, deps: SlashDispatchDeps): Promise<void> {
  const value = args.trim().toLowerCase();
  if (value.length === 0) {
    try {
      const current = await deps.getSensitivity();
      deps.addSystemMessage(`Sensitivity is set to ${current}. ${SENSITIVITY_USAGE}`, "info");
    } catch {
      deps.addSystemMessage("Could not read sensitivity right now.", "error");
    }
    return;
  }
  if (!SENSITIVITY_VALUES.has(value)) {
    deps.addSystemMessage(SENSITIVITY_USAGE, "error");
    return;
  }
  try {
    await deps.setSensitivity(value);
    deps.addSystemMessage(`Sensitivity set to ${value}.`, "info");
  } catch {
    deps.addSystemMessage("Could not update sensitivity right now.", "error");
  }
}

export function useSlashDispatch() {
  const sendMessage = useSendMessage();
  const addSystemMessage = useChatStore((state) => state.addSystemMessage);
  const addArtifactMessage = useChatStore((state) => state.addArtifactMessage);
  const invalidateSidebar = useInvalidateSidebar();

  return useCallback(
    async (raw: string) => {
      const { command, args } = parseRawSlash(raw);
      await dispatchSlashCommand(command, args, {
        sendMessage,
        addSystemMessage,
        addArtifactMessage,
        invalidateSidebar,
        undoLastTransaction: async () => {
          const r = await safeInvoke<string>("undo_last_transaction");
          if (!r.ok) throw r.error;
        },
        getAIDefaults: async () => {
          const r = await safeInvoke<Record<string, unknown>>("get_ai_defaults");
          if (!r.ok) throw r.error;
          return r.value;
        },
        setAIDefault: async (key: string, value: string) => {
          const r = await safeInvoke<void>("set_ai_default", { args: { key, value } });
          if (!r.ok) throw r.error;
        },
        getSensitivity: async () => {
          const r = await safeInvoke<string>("get_sensitivity");
          if (!r.ok) throw r.error;
          return r.value;
        },
        setSensitivity: async (value: string) => {
          const r = await safeInvoke<void>("set_sensitivity", {
            args: { sensitivity: value },
          });
          if (!r.ok) throw r.error;
        },
      });
    },
    [addArtifactMessage, addSystemMessage, invalidateSidebar, sendMessage],
  );
}

export { UNKNOWN_COMMAND_MESSAGE };
