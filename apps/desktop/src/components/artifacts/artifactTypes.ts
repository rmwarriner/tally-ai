export interface LedgerRow {
  date: number;
  payee: string;
  amount_cents: number;
  side: "debit" | "credit";
}

export interface BalanceNode {
  account_name: string;
  balance_cents: number;
  depth: number;
  is_subtotal: boolean;
}
