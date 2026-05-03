import { create } from "zustand";

type RefreshStore = {
  dashboardKey: number;
  importKey: number;
  invoicesKey: number;

  triggerDashboardRefresh: () => void;
  triggerImportRefresh: () => void;
  triggerInvoicesRefresh: () => void;
};

export const useRefreshStore = create<RefreshStore>((set) => ({
  dashboardKey: 0,
  importKey: 0,
  invoicesKey: 0,

  triggerDashboardRefresh: () =>
    set((s) => ({ dashboardKey: s.dashboardKey + 1 })),
  triggerImportRefresh: () =>
    set((s) => ({ importKey: s.importKey + 1 })),
  triggerInvoicesRefresh: () =>
    set((s) => ({ invoicesKey: s.invoicesKey + 1 })),
}));
