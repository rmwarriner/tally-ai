export const SLASH_COMMANDS = [
  { name: "/budget", description: "Show envelope budget status for the current month" },
  { name: "/balance", description: "Show account balances" },
  { name: "/recent", description: "List recent transactions" },
  { name: "/fix", description: "Correct a transaction by description or ID" },
  { name: "/undo", description: "Undo the last AI-posted transaction" },
  { name: "/help", description: "Show available commands and tips" },
  { name: "/defaults", description: "View or change AI entry defaults" },
] as const;

export type SlashCommand = (typeof SLASH_COMMANDS)[number];
