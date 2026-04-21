import { create } from "zustand";

export type SidebarState = "open" | "icon" | "hidden";

interface UIStore {
  sidebarState: SidebarState;
  toggleSidebar: () => void;
}

const NEXT_STATE: Record<SidebarState, SidebarState> = {
  open: "icon",
  icon: "hidden",
  hidden: "open",
};

export const useUIStore = create<UIStore>((set) => ({
  sidebarState: "open",
  toggleSidebar: () => {
    set((state) => ({ sidebarState: NEXT_STATE[state.sidebarState] }));
  },
}));
