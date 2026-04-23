import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

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
    queryFn: async () => invoke<AccountBalance[]>("get_account_balances"),
    staleTime: 10_000,
  });
}

export function useEnvelopeStatuses() {
  return useQuery({
    queryKey: ["sidebar", "envelopes"],
    queryFn: async () => invoke<EnvelopeStatus[]>("get_current_envelope_periods"),
    staleTime: 10_000,
  });
}

export function usePendingTransactions() {
  return useQuery({
    queryKey: ["sidebar", "pending"],
    queryFn: async () => invoke<ComingUpTxn[]>("get_pending_transactions"),
    staleTime: 10_000,
  });
}
