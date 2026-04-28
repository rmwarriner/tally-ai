import { useQuery } from "@tanstack/react-query";

import { safeInvoke } from "../lib/safeInvoke";

export interface AccountBalance {
  id: string;
  name: string;
  type: "asset" | "liability";
  balance_cents: number;
}

export interface EnvelopeStatus {
  envelope_id: string;
  name: string;
  allocated_cents: number;
  spent_cents: number;
}

export interface ComingUpTxn {
  id: string;
  txn_date: number;
  status?: "pending" | "posted";
  payee?: string;
  memo?: string;
  amount_cents: number;
}

// Back-compat alias; remove once no callers reference PendingTxn.
export type PendingTxn = ComingUpTxn;

export function useAccountBalances() {
  return useQuery({
    queryKey: ["sidebar", "accounts"],
    queryFn: async () => {
      const r = await safeInvoke<AccountBalance[]>("get_account_balances");
      if (!r.ok) throw r.error;
      return r.value;
    },
    staleTime: 10_000,
  });
}

export function useEnvelopeStatuses() {
  return useQuery({
    queryKey: ["sidebar", "envelopes"],
    queryFn: async () => {
      const r = await safeInvoke<EnvelopeStatus[]>("get_current_envelope_periods");
      if (!r.ok) throw r.error;
      return r.value;
    },
    staleTime: 10_000,
  });
}

export function usePendingTransactions() {
  return useQuery({
    queryKey: ["sidebar", "pending"],
    queryFn: async () => {
      const r = await safeInvoke<ComingUpTxn[]>("get_pending_transactions");
      if (!r.ok) throw r.error;
      return r.value;
    },
    staleTime: 10_000,
  });
}
