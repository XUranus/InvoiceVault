import React from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportJob, ImportJobListResult } from "../types";
import {
  getInvoiceIdByRawFile,
  importFiles,
  listImportJobs,
  recognizeRawFile,
  rawFileHasInvoices,
} from "../api";
import { importStatusMeta, toneClass } from "../status";
import { useAppStore } from "../stores/appStore";
import { useLlmStore } from "../stores/llmStore";
import { useRefreshStore } from "../stores/refreshStore";
import { Paperclip } from "lucide-react";
import { useNavigateToInvoice } from "../hooks/useNavigateToInvoice";

export function ImportPage() {
  const isDraggingFiles = useAppStore((s) => s.isDraggingFiles);
  const llm = useLlmStore((s) => s.llm);
  const auditEnabled = useLlmStore((s) => s.auditEnabled);
  const refreshKey = useRefreshStore((s) => s.importKey);
  const onInvoicesAdded = useAppStore((s) => s.refreshInvoices);
  const onNavigateToInvoice = useNavigateToInvoice();
  const navigate = useNavigate();
  const setError = useAppStore((s) => s.setError);
  const [isImporting, setIsImporting] = React.useState(false);
  const [recognizingJobId, setRecognizingJobId] = React.useState<number | null>(null);
  const [expandedJobId, setExpandedJobId] = React.useState<number | null>(null);
  const [result, setResult] = React.useState<ImportJobListResult | null>(null);
  const [page, setPage] = React.useState(1);
  const [recognizedFileIds, setRecognizedFileIds] = React.useState<Set<number>>(new Set());
  const [optimisticJobs, setOptimisticJobs] = React.useState<ImportJob[]>([]);
  const pageSize = 5;

  const fetchJobs = React.useCallback(async (p: number) => {
    try {
      const r = await listImportJobs(p, pageSize);
      setResult(r);
      setPage(r.page);
      // Check which files already have invoices
      const recognized = new Set<number>();
      for (const job of r.jobs) {
        if (job.raw_file_id) {
          try {
            const hasInvoices = await rawFileHasInvoices(job.raw_file_id);
            if (hasInvoices) recognized.add(job.id);
          } catch { /* ignore */ }
        }
      }
      setRecognizedFileIds(recognized);
    } catch (err) {
      setError(String(err));
    }
  }, [setError]);

  React.useEffect(() => {
    fetchJobs(1);
  }, [refreshKey, fetchJobs]);

  React.useEffect(() => {
    if (!result?.jobs.some(isActiveImportJob)) return;

    const timer = window.setInterval(() => {
      fetchJobs(page);
    }, 2000);
    return () => window.clearInterval(timer);
  }, [fetchJobs, page, result?.jobs]);

  const doImport = React.useCallback(
    async (paths: string[]) => {
      setIsImporting(true);
      setOptimisticJobs(paths.map(createOptimisticJob));
      try {
        await importFiles(paths);
        // Auto-recognition is triggered by backend after import
        // Poll for updated job list after a short delay
        await fetchJobs(1);
        setTimeout(() => fetchJobs(1), 1500);
      } catch (err) {
        setError(String(err));
      } finally {
        setIsImporting(false);
        setOptimisticJobs([]);
      }
    },
    [fetchJobs, setError],
  );

  const handlePickFiles = async () => {
    if (isImporting) return;
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        filters: [
          { name: "发票文件", extensions: ["pdf", "png", "jpg", "jpeg"] },
        ],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return;
      await doImport(paths);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDropAreaKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    handlePickFiles();
  };

  const handleRecognize = async (job: ImportJob) => {
    if (!job.raw_file_id) {
      setError("该导入任务没有可识别的 RAW 文件。");
      return;
    }
    if (!llm.config.apiKey.trim()) {
      setError("请先在设置中填写 LLM API Key。");
      return;
    }
    setRecognizingJobId(job.id);
    try {
      await recognizeRawFile({
        raw_file_id: job.raw_file_id,
        config: {
          base_url: llm.config.baseUrl,
          api_key: llm.config.apiKey,
          model: llm.config.model,
          timeout_seconds: 90,
        },
      });
      onInvoicesAdded();
      setExpandedJobId(job.id);
      fetchJobs(page);
    } catch (err) {
      setError(String(err));
      fetchJobs(page);
    } finally {
      setRecognizingJobId(null);
    }
  };

  const handleOpenImportedInvoice = async (job: ImportJob) => {
    if (job.invoice_id) {
      onNavigateToInvoice(job.invoice_id, "/import");
      return;
    }
    if (!job.raw_file_id) {
      setError("该导入任务没有可打开的发票详情。");
      return;
    }
    try {
      const invoiceId = await getInvoiceIdByRawFile(job.raw_file_id);
      if (invoiceId) {
        onNavigateToInvoice(invoiceId, "/import");
      } else {
        setError("该文件还没有生成发票详情。");
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleOpenDuplicateInvoice = async (job: ImportJob) => {
    if (!job.raw_file_id) {
      setError("该重复文件没有关联的原始文件。");
      return;
    }
    try {
      const invoiceId = await getInvoiceIdByRawFile(job.raw_file_id);
      if (invoiceId) {
        onNavigateToInvoice(invoiceId, "/import");
      } else {
        setError("该重复文件还没有生成发票详情。");
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const allJobs = result?.jobs ?? [];
  const visibleJobs = [...optimisticJobs, ...allJobs];
  const activeJobs = allJobs.filter(isActiveImportJob);
  const visibleActiveJobs = [
    ...optimisticJobs,
    ...activeJobs,
  ];
  const completedJobs = allJobs.filter((j) => !isActiveImportJob(j));

  return (
    <div className="page">
      <div className="page-header">
        <h2 className="page-title" style={{ margin: 0 }}>导入发票</h2>
        <button className="btn-small" onClick={() => navigate("/import/sources")}>
          数据源
        </button>
      </div>

      <div className="import-zone">
        <button
          type="button"
          className={`drop-area ${isDraggingFiles ? "drop-area-active" : ""} ${isImporting ? "drop-area-disabled" : ""}`}
          aria-disabled={isImporting}
          disabled={isImporting}
          onClick={handlePickFiles}
          onKeyDown={handleDropAreaKeyDown}
        >
          <Paperclip size={32} className="drop-icon" />
          <p>{isDraggingFiles ? "松开以导入文件" : "拖入 PDF / PNG / JPG / JPEG 文件"}</p>
          <span className="drop-hint">也可以点击此区域选择文件</span>
        </button>
      </div>

      {visibleActiveJobs.length > 0 ? (
        <div className="section">
          <h3>进行中</h3>
          <div className="job-list">
            {visibleActiveJobs.map((job) => (
              <JobRow
                key={job.id}
                job={job}
                recognizingJobId={recognizingJobId}
                expandedJobId={expandedJobId}
                onRecognize={handleRecognize}
                onToggleExpand={setExpandedJobId}
                isRecognized={recognizedFileIds.has(job.id)}
              />
            ))}
          </div>
        </div>
      ) : null}

      <div className="section">
        <h3>
          导入历史
          {result ? (
            <span className="badge">
              {completedJobs.length}
            </span>
          ) : null}
        </h3>
        {completedJobs.length === 0 ? (
          <p className="muted">暂无导入记录。</p>
        ) : (
          <>
            <ImportHistoryTable
              jobs={completedJobs}
              expandedJobId={expandedJobId}
              onToggleExpand={setExpandedJobId}
              onOpenImportedInvoice={handleOpenImportedInvoice}
              onOpenDuplicateInvoice={handleOpenDuplicateInvoice}
            />
            {result && result.total_pages > 1 ? (
              <div className="pagination" style={{ marginTop: 12, justifyContent: "center" }}>
                <button
                  className="page-btn"
                  disabled={page <= 1}
                  onClick={() => fetchJobs(page - 1)}
                >
                  上一页
                </button>
                <span className="page-info">
                  {result.page} / {result.total_pages}（共 {result.total_count} 条）
                </span>
                <button
                  className="page-btn"
                  disabled={page >= result.total_pages}
                  onClick={() => fetchJobs(page + 1)}
                >
                  下一页
                </button>
              </div>
            ) : null}
          </>
        )}
      </div>
    </div>
  );
}

function createOptimisticJob(path: string, index: number): ImportJob {
  const name = path.split(/[\\/]/).pop() || path;
  const now = String(Date.now());
  return {
    id: -(Date.now() + index),
    raw_file_id: null,
    invoice_id: null,
    source_path: path,
    original_name: name,
    current_name: null,
    status: "importing",
    sha256: null,
    storage_path: null,
    mime_type: inferMimeType(name),
    error_message: null,
    source_type: "manual",
    created_at: now,
    updated_at: now,
  };
}

function ImportHistoryTable({
  jobs,
  expandedJobId,
  onToggleExpand,
  onOpenImportedInvoice,
  onOpenDuplicateInvoice,
}: {
  jobs: ImportJob[];
  expandedJobId: number | null;
  onToggleExpand: (id: number | null) => void;
  onOpenImportedInvoice: (job: ImportJob) => void;
  onOpenDuplicateInvoice: (job: ImportJob) => void;
}) {
  return (
    <div className="import-history-table-wrap">
      <table className="import-history-table">
        <thead>
          <tr>
            <th>文件</th>
            <th>来源</th>
            <th>时间</th>
            <th>类型</th>
            <th>状态</th>
          </tr>
        </thead>
        <tbody>
          {jobs.map((job) => (
            <ImportHistoryRow
              key={job.id}
              job={job}
              expanded={expandedJobId === job.id}
              onToggleExpand={onToggleExpand}
              onOpenImportedInvoice={onOpenImportedInvoice}
              onOpenDuplicateInvoice={onOpenDuplicateInvoice}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ImportHistoryRow({
  job,
  expanded,
  onToggleExpand,
  onOpenImportedInvoice,
  onOpenDuplicateInvoice,
}: {
  job: ImportJob;
  expanded: boolean;
  onToggleExpand: (id: number | null) => void;
  onOpenImportedInvoice: (job: ImportJob) => void;
  onOpenDuplicateInvoice: (job: ImportJob) => void;
}) {
  const meta = importStatusMeta(job.status);
  const fileTypeLabel = formatFileType(job.mime_type, job.original_name ?? job.source_path);
  const isDuplicate = job.status === "duplicate";
  const canOpenInvoice =
    ["imported", "completed", "recognized"].includes(job.status) && Boolean(job.invoice_id);
  const isFailed = job.status === "failed";

  return (
    <>
      <tr
        className={`import-history-row ${expanded ? "import-history-row-expanded" : ""}`}
        onClick={() => onToggleExpand(expanded ? null : job.id)}
      >
        <td className="import-history-file-cell">
          <strong>{jobDisplayName(job)}</strong>
        </td>
        <td>
          <span className="mini-tag tag-tone-neutral">{sourceTypeLabel(job.source_type)}</span>
        </td>
        <td className="import-history-time-cell">
          {formatImportTime(job.updated_at || job.created_at)}
        </td>
        <td>
          <span className="mini-tag tag-tone-neutral">{fileTypeLabel}</span>
        </td>
        <td>
          {isFailed ? (
            <span
              className="status-tag tag-tone-danger"
              title={job.error_message ?? undefined}
            >
              识别失败
            </span>
          ) : isDuplicate ? (
            <button
              className={`status-tag status-tag-button ${toneClass(meta.tone)}`}
              onClick={(e) => {
                e.stopPropagation();
                onOpenDuplicateInvoice(job);
              }}
              type="button"
              title="点击查看重复文件的发票详情"
            >
              文件重复
            </button>
          ) : canOpenInvoice ? (
            <button
              className={`status-tag status-tag-button ${toneClass(meta.tone)}`}
              onClick={(e) => {
                e.stopPropagation();
                onOpenImportedInvoice(job);
              }}
              type="button"
              title="点击查看发票详情"
            >
              {meta.label}
            </button>
          ) : (
            <span className={`status-tag ${toneClass(meta.tone)}`}>
              {meta.label}
            </span>
          )}
        </td>
      </tr>
      {expanded ? (
        <tr className="import-history-detail-row">
          <td colSpan={4}>
            <JobDetail job={job} />
          </td>
        </tr>
      ) : null}
    </>
  );
}

function JobRow({
  job,
  recognizingJobId,
  expandedJobId,
  onRecognize,
  onToggleExpand,
  isRecognized,
}: {
  job: ImportJob;
  recognizingJobId: number | null;
  expandedJobId: number | null;
  onRecognize: (job: ImportJob) => void;
  onToggleExpand: (id: number | null) => void;
  isRecognized: boolean;
}) {
  const meta = importStatusMeta(job.status);
  const isBusy = isActiveImportJob(job) || recognizingJobId === job.id;
  const fileTypeLabel = formatFileType(job.mime_type, job.original_name ?? job.source_path);
  const timeLabel = formatImportTime(job.updated_at || job.created_at);

  return (
    <article
      className={`job-card ${expandedJobId === job.id ? "job-card-expanded" : ""}`}
      onClick={() =>
        onToggleExpand(expandedJobId === job.id ? null : job.id)
      }
    >
      <div className="job-card-header">
        <div className="job-card-info">
          <strong>{jobDisplayName(job)}</strong>
          <span className="job-meta">
            {timeLabel}
          </span>
        </div>
        <div className="job-card-actions" onClick={(e) => e.stopPropagation()}>
          <span className="mini-tag tag-tone-neutral">{fileTypeLabel}</span>
          <span className={`status-tag ${toneClass(meta.tone)} ${isBusy ? "status-tag-busy" : ""}`}>
            {isBusy ? <span className="inline-spinner" /> : null}
            {job.status === "duplicate" ? "文件重复" : meta.label}
          </span>
          {canRecognizeJob(job) ? (
            <button
              className="btn-small"
              disabled={recognizingJobId !== null}
              onClick={() => onRecognize(job)}
            >
              {recognizingJobId === job.id ? "识别中..." : isRecognized ? "重新识别" : "识别"}
            </button>
          ) : null}
        </div>
      </div>

      {expandedJobId === job.id ? (
        <div className="job-card-detail">
          <JobDetail job={job} />
        </div>
      ) : null}
    </article>
  );
}

function JobDetail({ job }: { job: ImportJob }) {
  const [copiedField, setCopiedField] = React.useState<string | null>(null);

  const copyText = async (e: React.MouseEvent, field: string, value: string | null) => {
    e.stopPropagation();
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopiedField(field);
      window.setTimeout(() => setCopiedField(null), 1200);
    } catch {
      setCopiedField(null);
    }
  };

  return (
    <dl className="job-detail-list">
      <dt>来源</dt>
      <dd>{sourceTypeLabel(job.source_type)}</dd>
      <dt>源路径</dt>
      <dd>
        <CopyableText
          field="source_path"
          value={job.source_path}
          label="点击复制源路径"
          copiedField={copiedField}
          onCopy={copyText}
        />
      </dd>
      {job.current_name ? (
        <>
          <dt>存储名</dt>
          <dd>
            <CopyableText
              field="current_name"
              value={job.current_name}
              label="点击复制存储名"
              copiedField={copiedField}
              onCopy={copyText}
            />
          </dd>
        </>
      ) : null}
      {job.sha256 ? (
        <>
          <dt>SHA256</dt>
          <dd>
            <CopyableText
              field="sha256"
              value={job.sha256}
              label="点击复制 SHA256"
              copiedField={copiedField}
              onCopy={copyText}
              mono
            />
          </dd>
        </>
      ) : null}
      {job.error_message ? (
        <>
          <dt>错误</dt>
          <dd className="text-error">{job.error_message}</dd>
        </>
      ) : null}
      <dt>创建时间</dt>
      <dd>{formatImportTime(job.created_at)}</dd>
      <dt>更新时间</dt>
      <dd>{formatImportTime(job.updated_at)}</dd>
    </dl>
  );
}

function CopyableText({
  field,
  value,
  label,
  copiedField,
  onCopy,
  mono = false,
}: {
  field: string;
  value: string;
  label: string;
  copiedField: string | null;
  onCopy: (event: React.MouseEvent, field: string, value: string) => void;
  mono?: boolean;
}) {
  return (
    <>
      <button
        className={`copy-text-btn ${mono ? "mono" : ""}`}
        onClick={(e) => onCopy(e, field, value)}
        title={label}
      >
        {value}
      </button>
      {copiedField === field ? <span className="copy-hint">已复制</span> : null}
    </>
  );
}

function jobDisplayName(job: ImportJob): string {
  return job.original_name ?? job.source_path.split(/[\\/]/).pop() ?? job.source_path;
}

function sourceTypeLabel(sourceType: string): string {
  const labels: Record<string, string> = {
    manual: "手动上传",
    watcher: "文件夹监听",
    email: "邮件导入",
  };
  return labels[sourceType] ?? sourceType;
}

function isActiveImportJob(job: ImportJob): boolean {
  return ["importing", "pending", "processing", "recognizing"].includes(job.status);
}

function canRecognizeJob(job: ImportJob) {
  return (
    job.raw_file_id !== null &&
    job.status === "failed" &&
    (job.mime_type === "image/png" ||
      job.mime_type === "image/jpeg" ||
      job.mime_type === "application/pdf")
  );
}

function inferMimeType(name: string): string | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".pdf")) return "application/pdf";
  if (lower.endsWith(".png")) return "image/png";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  return null;
}

function formatFileType(mimeType: string | null, name: string): string {
  const mime = mimeType ?? inferMimeType(name);
  if (mime === "application/pdf") return "PDF";
  if (mime === "image/png") return "PNG";
  if (mime === "image/jpeg") return "JPEG";
  return "文件";
}

function formatImportTime(value: string): string {
  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 1000000000000) {
    return new Date(numericValue).toLocaleString();
  }
  return value;
}

export default ImportPage;
