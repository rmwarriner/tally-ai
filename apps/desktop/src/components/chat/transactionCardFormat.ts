import { formatCents } from "../../utils/formatCents";

const DATE_FORMAT = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  year: "numeric",
});

export function formatTransactionDate(unixMs: number): string {
  return DATE_FORMAT.format(new Date(unixMs));
}

export function formatTransactionAriaLabel(payee: string, amountCents: number): string {
  return `Transaction: ${payee}, ${formatCents(amountCents)}`;
}
