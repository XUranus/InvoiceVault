import { create } from "zustand";
import type { AppHealth, Invoice } from "../types";
import { countUnviewedInvoices, getAppHealth, searchInvoices, getUnreadEventCount } from "../api";

type AppStore = {
  health: AppHealth | null;
  error: string | null;
  theme: "light" | "dark";
  invoices: Invoice[];
  isDraggingFiles: boolean;
  unreadEventCount: number;
  importBadgeCount: number;
  invoiceBadgeCount: number;
  sidebarCollapsed: boolean;
  showOnboarding: boolean;

  setHealth: (h: AppHealth) => void;
  setError: (err: string) => void;
  clearError: () => void;
  toggleTheme: () => void;
  setInvoices: (list: Invoice[]) => void;
  setIsDraggingFiles: (v: boolean) => void;
  setUnreadEventCount: (n: number) => void;
  setImportBadgeCount: (n: number) => void;
  setInvoiceBadgeCount: (n: number) => void;
  toggleSidebar: () => void;
  setShowOnboarding: (v: boolean) => void;
  dismissOnboarding: () => void;
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
  unreadEventCount: 0,
  importBadgeCount: 0,
  invoiceBadgeCount: 0,
  sidebarCollapsed:
    localStorage.getItem("sidebarCollapsed") === "true",
  showOnboarding: false,

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
  setUnreadEventCount: (unreadEventCount) =>
    set({ unreadEventCount }),
  setImportBadgeCount: (importBadgeCount) => set({ importBadgeCount }),
  setInvoiceBadgeCount: (invoiceBadgeCount) => set({ invoiceBadgeCount }),
  toggleSidebar: () => {
    const next = !get().sidebarCollapsed;
    localStorage.setItem("sidebarCollapsed", String(next));
    set({ sidebarCollapsed: next });
  },
  setShowOnboarding: (showOnboarding) => set({ showOnboarding }),
  dismissOnboarding: () => {
    localStorage.setItem("onboarding_dismissed", "1");
    set({ showOnboarding: false });
  },

  initialize: async () => {
    try {
      const health = await getAppHealth();
      set({ health });
    } catch (err) {
      set({ error: String(err) });
    }
    try {
      const [result, invoiceBadgeCount] = await Promise.all([
        searchInvoices({ page: 1, page_size: 100 }),
        countUnviewedInvoices(),
      ]);
      set({ invoices: result.invoices, invoiceBadgeCount });
    } catch (err) {
      set({ error: String(err) });
    }
    try {
      const unreadEventCount = await getUnreadEventCount();
      set({ unreadEventCount });
    } catch {
      // ignore
    }
  },

  refreshInvoices: async () => {
    try {
      const [result, invoiceBadgeCount] = await Promise.all([
        searchInvoices({ page: 1, page_size: 100 }),
        countUnviewedInvoices(),
      ]);
      set({ invoices: result.invoices, invoiceBadgeCount });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
