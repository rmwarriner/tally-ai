import type { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

import { safeInvoke } from "../lib/safeInvoke";
import { useChatStore, type ProactiveInsight } from "../stores/chatStore";
import { useOnboardingStore } from "../stores/onboardingStore";

export interface SessionOpenDeps {
  invoke?: typeof tauriInvoke;
}

/// Fires once per app mount, after onboarding completes. Asks the backend
/// for a morning briefing; if returned (sensitivity == proactive AND first
/// session of the household-local day), appends it as a proactive message.
export function useSessionOpen(deps: SessionOpenDeps = {}): void {
  const phase = useOnboardingStore((s) => s.phase);
  const appendInsight = useChatStore((s) => s.appendInsight);
  const firedRef = useRef(false);

  useEffect(() => {
    if (phase !== "complete" || firedRef.current) return;
    firedRef.current = true;

    void (async () => {
      const r = await safeInvoke<ProactiveInsight | null>(
        "session_open",
        undefined,
        { invoke: deps.invoke },
      );
      if (!r.ok) {
        // Briefing is best-effort; surface to console only.
        console.warn("session_open failed:", r.error);
        return;
      }
      if (r.value !== null) {
        appendInsight(r.value);
      }
    })();
  }, [phase, appendInsight, deps]);
}
