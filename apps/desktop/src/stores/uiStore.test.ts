import { beforeEach, describe, expect, it } from "vitest";

import { useUIStore } from "./uiStore";

describe("useUIStore", () => {
  beforeEach(() => {
    useUIStore.setState({ sidebarOpen: true });
  });

  it("starts open", () => {
    expect(useUIStore.getState().sidebarOpen).toBe(true);
  });

  it("toggleSidebar closes then reopens", () => {
    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarOpen).toBe(false);

    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarOpen).toBe(true);
  });
});
