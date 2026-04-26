// eslint-disable-next-line no-restricted-imports
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { RecoveryError, RecoveryAction } from "@tally/core-types";
import { useChatStore } from "../stores/chatStore";

export type Result<T> =
  | { ok: true; value: T }
  | { ok: false; error: RecoveryError };

interface Deps {
  invoke?: typeof tauriInvoke;
  dispatchAdvisory?: (err: RecoveryError) => void;
}

const DEFAULT_RECOVERY: [RecoveryAction, ...RecoveryAction[]] = [
  { kind: "SHOW_HELP", label: "Get help", is_primary: true },
  { kind: "DISCARD", label: "Discard", is_primary: false },
];

function isRecoveryError(value: unknown): value is RecoveryError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as { message?: unknown; recovery?: unknown };
  return (
    typeof v.message === "string" &&
    Array.isArray(v.recovery) &&
    v.recovery.length > 0
  );
}

function normalize(raw: unknown): RecoveryError {
  if (isRecoveryError(raw)) {
    return raw;
  }
  if (typeof raw === "string") {
    return { message: raw, recovery: DEFAULT_RECOVERY };
  }
  return { message: "Something went wrong.", recovery: DEFAULT_RECOVERY };
}

export async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  deps: Deps = {},
): Promise<Result<T>> {
  const invoke = deps.invoke ?? tauriInvoke;
  try {
    const value = await invoke<T>(cmd, args);
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalize(raw) };
  }
}

export async function safeInvokeOrAdvise<T>(
  cmd: string,
  args?: Record<string, unknown>,
  deps: Deps = {},
): Promise<T | null> {
  const result = await safeInvoke<T>(cmd, args, deps);
  if (result.ok) return result.value;
  const dispatch = deps.dispatchAdvisory ?? defaultDispatch;
  dispatch(result.error);
  return null;
}

function defaultDispatch(err: RecoveryError): void {
  // Dispatched as a system advisory message via the chat store.
  // Task 12 adds appendAdvisory; until then this is a no-op via optional chaining.
  const store = useChatStore.getState() as unknown as {
    appendAdvisory?: (err: RecoveryError) => void;
  };
  store.appendAdvisory?.(err);
}
