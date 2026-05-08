// Playwright fixture extension: installs a mocked `invoke` into the page
// before the React app boots, and exposes recorded calls back to the spec.
//
// `@tauri-apps/api/core` dispatches via `window.__TAURI_INTERNALS__.invoke`.
// We define that global in an `addInitScript` so the very first invoke
// (from `useChatPersistence`/`buildOnboardingHandler`) hits our mock instead
// of throwing "TAURI_INTERNALS undefined" in a browser tab.
//
// Handlers are pure data — no function-eval across the page boundary:
//   • a literal value/object → returned as-is
//   • `{ __sequence: [...] }` → returned in order, repeating the last one
//   • `{ __reject: true, message, recovery }` → invoke promise rejects

import { test as base, expect } from "@playwright/test";

import type { MockHandler, MockResponses } from "./fixtures/responses";

interface InvokeCall {
  cmd: string;
  args: unknown;
}

interface MockInvokeFixtures {
  /** Configures the mock for this test. Call before `page.goto("/")`. */
  setupMock: (responses: MockResponses) => Promise<void>;
  /** Returns recorded invoke calls in the order they were made. */
  getInvokeCalls: () => Promise<InvokeCall[]>;
  /** Updates a single command's response between actions. */
  patchMock: (cmd: string, handler: MockHandler) => Promise<void>;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (cmd: string, args?: unknown, options?: unknown) => Promise<unknown>;
    };
    __E2E__?: {
      setResponses: (next: Record<string, unknown>) => void;
      patchResponse: (cmd: string, handler: unknown) => void;
      getCalls: () => InvokeCall[];
      counts: Record<string, number>;
    };
  }
}

export const test = base.extend<MockInvokeFixtures>({
  setupMock: async ({ page }, use) => {
    let installed = false;

    const setup = async (responses: MockResponses) => {
      const serializable = JSON.parse(JSON.stringify(responses));

      if (installed) {
        await page.evaluate((next) => {
          window.__E2E__?.setResponses(next as Record<string, unknown>);
        }, serializable);
        return;
      }

      await page.addInitScript((initial) => {
        const calls: InvokeCall[] = [];
        const counts: Record<string, number> = {};
        let current: Record<string, unknown> = initial as Record<string, unknown>;

        function pickFromHandler(handler: unknown, idx: number): unknown {
          if (handler && typeof handler === "object") {
            const h = handler as { __sequence?: unknown[] };
            if (Array.isArray(h.__sequence)) {
              const seq = h.__sequence;
              return seq[Math.min(idx, seq.length - 1)];
            }
          }
          return handler;
        }

        const invoke = async (cmd: string, args: unknown = {}) => {
          calls.push({ cmd, args });
          counts[cmd] = (counts[cmd] ?? 0) + 1;

          if (!(cmd in current)) {
            return Promise.reject({
              message: `[mock] No response configured for "${cmd}"`,
              recovery: [{ kind: "SHOW_HELP", label: "Get help", is_primary: true }],
            });
          }

          const picked = pickFromHandler(current[cmd], counts[cmd] - 1);
          if (picked && typeof picked === "object" && "__reject" in (picked as object)) {
            const err = picked as { message: string; recovery: unknown };
            return Promise.reject({ message: err.message, recovery: err.recovery });
          }
          return picked;
        };

        window.__TAURI_INTERNALS__ = { invoke };
        window.__E2E__ = {
          setResponses: (next) => {
            current = next as Record<string, unknown>;
          },
          patchResponse: (cmd, handler) => {
            current[cmd] = handler;
          },
          getCalls: () => calls.slice(),
          counts,
        };
      }, serializable);

      installed = true;
    };

    await use(setup);
  },

  getInvokeCalls: async ({ page }, use) => {
    await use(async () => {
      return (await page.evaluate(() => window.__E2E__?.getCalls() ?? [])) as InvokeCall[];
    });
  },

  patchMock: async ({ page }, use) => {
    await use(async (cmd, handler) => {
      const serializable = JSON.parse(JSON.stringify(handler));
      await page.evaluate(
        ({ c, h }) => window.__E2E__?.patchResponse(c, h),
        { c: cmd, h: serializable },
      );
    });
  },
});

export { expect };
