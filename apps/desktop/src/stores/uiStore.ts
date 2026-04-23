import { create } from "zustand";

export type SidebarState = "open" | "icon" | "hidden";
export type ContextChipType = "account" | "envelope" | "date-range";

export interface ContextChip {
  id: string;
  type: ContextChipType;
  label: string;
}

interface UIStore {
  sidebarState: SidebarState;
  contextChips: ContextChip[];
  toggleSidebar: () => void;
  setContextChips: (chips: ContextChip[]) => void;
  removeContextChip: (id: string) => void;
}

const NEXT_STATE: Record<SidebarState, SidebarState> = {
  open: "icon",
  icon: "hidden",
  hidden: "open",
};

export const useUIStore = create<UIStore>((set) => ({
  sidebarState: "open",
  contextChips: [],
  toggleSidebar: () => {
    set((state) => ({ sidebarState: NEXT_STATE[state.sidebarState] }));
  },
  setContextChips: (chips) => {
    set({ contextChips: chips });
  },
  removeContextChip: (id) => {
    set((state) => ({
      contextChips: state.contextChips.filter((chip) => chip.id !== id),
    }));
  },
}));
