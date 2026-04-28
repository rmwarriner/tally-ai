import { describe, it, expect, vi } from "vitest";
import { safeInvoke, safeInvokeOrAdvise } from "./safeInvoke";
import type { RecoveryError } from "@tally/core-types";

describe("safeInvoke", () => {
  it("returns ok=true with value on success", async () => {
    const fakeInvoke = vi.fn().mockResolvedValue({ id: "x" });
    const r = await safeInvoke<{ id: string }>("get_thing", undefined, { invoke: fakeInvoke });
    expect(r).toEqual({ ok: true, value: { id: "x" } });
  });

  it("returns ok=false with structured RecoveryError on Tauri Err(RecoveryError)", async () => {
    const recoveryErr: RecoveryError = {
      message: "Account does not exist",
      recovery: [{ kind: "CREATE_MISSING", label: "Create", is_primary: true }],
    };
    const fakeInvoke = vi.fn().mockRejectedValue(recoveryErr);
    const r = await safeInvoke("create_thing", undefined, { invoke: fakeInvoke });
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.message).toBe("Account does not exist");
      expect(r.error.recovery[0].kind).toBe("CREATE_MISSING");
    }
  });

  it("normalizes string errors (panic / IPC) into ShowHelp + Discard", async () => {
    const fakeInvoke = vi.fn().mockRejectedValue("ipc connection failed");
    const r = await safeInvoke("anything", undefined, { invoke: fakeInvoke });
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.message).toBe("ipc connection failed");
      const kinds = r.error.recovery.map((a) => a.kind);
      expect(kinds).toContain("SHOW_HELP");
      expect(kinds).toContain("DISCARD");
    }
  });

  it("normalizes unknown error shapes into a generic RecoveryError", async () => {
    const fakeInvoke = vi.fn().mockRejectedValue({ random: "junk" });
    const r = await safeInvoke("anything", undefined, { invoke: fakeInvoke });
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.message).toBeTruthy();
      expect(r.error.recovery.length).toBeGreaterThan(0);
    }
  });
});

describe("safeInvokeOrAdvise", () => {
  it("returns the value on success", async () => {
    const fakeInvoke = vi.fn().mockResolvedValue(42);
    const dispatch = vi.fn();
    const v = await safeInvokeOrAdvise<number>("get_n", undefined, {
      invoke: fakeInvoke,
      dispatchAdvisory: dispatch,
    });
    expect(v).toBe(42);
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("returns null and dispatches an advisory on error", async () => {
    const recoveryErr: RecoveryError = {
      message: "Bang",
      recovery: [{ kind: "SHOW_HELP", label: "Help", is_primary: true }],
    };
    const fakeInvoke = vi.fn().mockRejectedValue(recoveryErr);
    const dispatch = vi.fn();
    const v = await safeInvokeOrAdvise("x", undefined, {
      invoke: fakeInvoke,
      dispatchAdvisory: dispatch,
    });
    expect(v).toBeNull();
    expect(dispatch).toHaveBeenCalledWith(recoveryErr);
  });
});
