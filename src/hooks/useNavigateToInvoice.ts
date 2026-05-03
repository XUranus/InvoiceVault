import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { useRefreshStore } from "../stores/refreshStore";

export function useNavigateToInvoice() {
  const navigate = useNavigate();
  const triggerInvoicesRefresh = useRefreshStore(
    (s) => s.triggerInvoicesRefresh,
  );

  return useCallback(
    (id: number) => {
      sessionStorage.setItem("focusInvoiceId", String(id));
      triggerInvoicesRefresh();
      navigate("/invoices");
    },
    [navigate, triggerInvoicesRefresh],
  );
}
