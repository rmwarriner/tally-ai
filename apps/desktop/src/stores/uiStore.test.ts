import { beforeEach, describe, expect, it } from "vitest";

import { useUIStore } from "./uiStore";

describe("useUIStore", () => {
  beforeEach(() => {
    useUIStore.setState({ sidebarState: "open" });
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
});
