import { beforeEach, describe, expect, it } from "vitest";

import { useUIStore } from "./uiStore";

describe("useUIStore", () => {
  beforeEach(() => {
    useUIStore.setState({ sidebarState: "open", contextChips: [] });
  });

  it("starts open", () => {
    expect(useUIStore.getState().sidebarState).toBe("open");
  });

  it("cycles open -> icon -> hidden -> open", () => {
    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarState).toBe("icon");

    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarState).toBe("hidden");

    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarState).toBe("open");
  });

  it("sets and removes context chips", () => {
    useUIStore.getState().setContextChips([
      { id: "c1", type: "account", label: "Checking" },
      { id: "c2", type: "envelope", label: "Groceries" },
    ]);
    expect(useUIStore.getState().contextChips).toHaveLength(2);

    useUIStore.getState().removeContextChip("c1");
    expect(useUIStore.getState().contextChips).toEqual([
      { id: "c2", type: "envelope", label: "Groceries" },
    ]);
  });
});
