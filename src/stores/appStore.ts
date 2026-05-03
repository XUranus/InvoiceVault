import { create } from "zustand";
import type { AppHealth, Invoice } from "../types";
import { getAppHealth, searchInvoices } from "../api";

type AppStore = {
  health: AppHealth | null;
  error: string | null;
  theme: "light" | "dark";
  invoices: Invoice[];
  isDraggingFiles: boolean;
  unreadNotificationCount: number;
  importBadgeCount: number;
  invoiceBadgeCount: number;

  setHealth: (h: AppHealth) => void;
  setError: (err: string) => void;
  clearError: () => void;
  toggleTheme: () => void;
  setInvoices: (list: Invoice[]) => void;
  setIsDraggingFiles: (v: boolean) => void;
  setUnreadNotificationCount: (n: number) => void;
  setImportBadgeCount: (n: number) => void;
  setInvoiceBadgeCount: (n: number) => void;
  incrementInvoiceBadgeCount: (n: number) => void;
  decrementInvoiceBadgeCount: () => void;
  initialize: () => Promise<void>;
  refreshInvoices: () => Promise<void>;
};

export const useAppStore = create<AppStore>((set, get) => ({
  health: null,
  error: null,
  theme:
    (localStorage.getItem("theme") as "light" | "dark" | null) ?? "light",
  invoices: [],
  isDraggingFiles: false,
  unreadNotificationCount: 0,
  importBadgeCount: 0,
  invoiceBadgeCount: 0,

  setHealth: (health) => set({ health }),
  setError: (error) => set({ error }),
  clearError: () => set({ error: null }),
  toggleTheme: () => {
    const next = get().theme === "dark" ? "light" : "dark";
    localStorage.setItem("theme", next);
    set({ theme: next });
  },
  setInvoices: (invoices) => set({ invoices }),
  setIsDraggingFiles: (isDraggingFiles) => set({ isDraggingFiles }),
  setUnreadNotificationCount: (unreadNotificationCount) =>
    set({ unreadNotificationCount }),
  setImportBadgeCount: (importBadgeCount) => set({ importBadgeCount }),
  setInvoiceBadgeCount: (invoiceBadgeCount) => set({ invoiceBadgeCount }),
  incrementInvoiceBadgeCount: (n) =>
    set((s) => ({ invoiceBadgeCount: s.invoiceBadgeCount + n })),
  decrementInvoiceBadgeCount: () =>
    set((s) => ({
      invoiceBadgeCount: Math.max(0, s.invoiceBadgeCount - 1),
    })),

  initialize: async () => {
    try {
      const health = await getAppHealth();
      set({ health });
    } catch (err) {
      set({ error: String(err) });
    }
    try {
      const result = await searchInvoices({ page: 1, page_size: 100 });
      set({ invoices: result.invoices });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  refreshInvoices: async () => {
    try {
      const result = await searchInvoices({ page: 1, page_size: 100 });
      set({ invoices: result.invoices });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
