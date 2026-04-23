import { invoke } from "@tauri-apps/api/core";
import { useCallback } from "react";

import { useChatStore } from "../stores/chatStore";
import { useSendMessage } from "./useSendMessage";

const UNKNOWN_COMMAND_MESSAGE = "Unknown command. Type /help to see available commands.";
const HELP_ARTIFACT_CONTENT = [
  "/budget    — Show envelope budget status for the current month",
  "/balance   — Show account balances",
  "/recent    — List recent transactions (add a number: /recent 20)",
  '/fix       — Correct a transaction: /fix "groceries on Tuesday was $45"',
  "/undo      — Undo the last AI-posted transaction",
  "/help      — Show this list",
  "/defaults  — View AI entry defaults (timezone, accounts)",
].join("\n");

type SystemTone = "info" | "error";

interface SlashDispatchDeps {
  sendMessage: (text: string) => void;
  addSystemMessage: (text: string, tone?: SystemTone) => void;
  addArtifactMessage: (title: string, content: string) => void;
  undoLastTransaction: () => Promise<void>;
  getAIDefaults: () => Promise<Record<string, unknown>>;
}

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
      try {
        const defaults = await deps.getAIDefaults();
        deps.addArtifactMessage("AI Defaults", formatDefaults(defaults));
      } catch {
        deps.addSystemMessage("Could not load AI defaults right now.", "error");
      }
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

export function useSlashDispatch() {
  const sendMessage = useSendMessage();
  const addSystemMessage = useChatStore((state) => state.addSystemMessage);
  const addArtifactMessage = useChatStore((state) => state.addArtifactMessage);

  return useCallback(
    async (raw: string) => {
      const { command, args } = parseRawSlash(raw);
      await dispatchSlashCommand(command, args, {
        sendMessage,
        addSystemMessage,
        addArtifactMessage,
        undoLastTransaction: () => invoke("undo_last_transaction"),
        getAIDefaults: () => invoke<Record<string, unknown>>("get_ai_defaults"),
      });
    },
    [addArtifactMessage, addSystemMessage, sendMessage],
  );
}

export { UNKNOWN_COMMAND_MESSAGE };
