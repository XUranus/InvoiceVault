import { invoke } from "@tauri-apps/api/core";
import type {
  AppHealth,
  ImportJob,
  Invoice,
  InvoiceSearchParams,
  InvoiceSearchResult,
  InvoiceDetail,
  UpdateInvoiceRequest,
  UpdateInvoiceResult,
  UpdateItemsRequest,
  InvoiceItemRow,
  LlmConnectionTestResult,
  RecognizeRawFileResult,
  DedupeCheckResult,
  ExportInvoicesRequest,
  ExportResult,
} from "./types";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}

export async function importFiles(paths: string[]): Promise<ImportJob[]> {
  return invoke<ImportJob[]>("import_files", { request: { paths } });
}

export async function listImportJobs(): Promise<ImportJob[]> {
  return invoke<ImportJob[]>("list_import_jobs");
}

export async function searchInvoices(
  params: InvoiceSearchParams,
): Promise<InvoiceSearchResult> {
  return invoke<InvoiceSearchResult>("search_invoices", { params });
}

export async function getInvoiceDetail(
  invoiceId: number,
): Promise<InvoiceDetail> {
  return invoke<InvoiceDetail>("get_invoice_detail", { invoiceId });
}

export async function updateInvoice(
  request: UpdateInvoiceRequest,
): Promise<UpdateInvoiceResult> {
  return invoke<UpdateInvoiceResult>("update_invoice", { request });
}

export async function updateInvoiceItems(
  request: UpdateItemsRequest,
): Promise<InvoiceItemRow[]> {
  return invoke<InvoiceItemRow[]>("update_invoice_items", { request });
}

export async function testLlmConnection(config: {
  base_url: string;
  api_key: string;
  model: string;
  timeout_seconds: number;
}): Promise<LlmConnectionTestResult> {
  return invoke<LlmConnectionTestResult>("test_llm_connection", { config });
}

export async function recognizeRawFile(request: {
  raw_file_id: number;
  config: {
    base_url: string;
    api_key: string;
    model: string;
    timeout_seconds: number;
  };
}): Promise<RecognizeRawFileResult> {
  return invoke<RecognizeRawFileResult>("recognize_raw_file", { request });
}

export async function checkInvoiceDuplicates(
  invoiceId: number,
): Promise<DedupeCheckResult> {
  return invoke<DedupeCheckResult>("check_invoice_duplicates", { invoiceId });
}

export async function resolveDuplicate(
  dedupeId: number,
  action: string,
): Promise<void> {
  return invoke<void>("resolve_duplicate", {
    request: { dedupe_id: dedupeId, action },
  });
}

export async function exportInvoices(
  request: ExportInvoicesRequest,
): Promise<ExportResult> {
  return invoke<ExportResult>("export_invoices", { request });
}
