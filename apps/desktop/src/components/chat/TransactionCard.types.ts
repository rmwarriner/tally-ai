export interface JournalLineDisplay {
  account_name: string;
  envelope_name?: string;
  amount_cents: number;
  side: "debit" | "credit";
}

export interface TransactionDisplay {
  id: string;
  payee: string;
  txn_date: number;
  amount_cents: number;
  account_name: string;
  lines: JournalLineDisplay[];
}

export type TransactionCardState = "posted" | "pending" | "voided" | "correction_pair";

export interface TransactionCardProps {
  state: TransactionCardState;
  transaction: TransactionDisplay;
  replacement?: TransactionDisplay;
  onSendMessage?: (message: string) => void;
}
