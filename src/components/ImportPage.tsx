import React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportJob, ImportJobListResult } from "../types";
import { importFiles, listImportJobs, recognizeRawFile, rawFileHasInvoices } from "../api";
import { importStatusMeta, toneClass } from "../status";

type Props = {
  isDraggingFiles: boolean;
  llmApiKey: string;
  llmBaseUrl: string;
  llmModel: string;
  refreshKey: number;
  onInvoicesAdded: () => void;
  onError: (error: string) => void;
};

export function ImportPage({
  isDraggingFiles,
  llmApiKey,
  llmBaseUrl,
  llmModel,
  refreshKey,
  onInvoicesAdded,
  onError,
}: Props) {
  const [isImporting, setIsImporting] = React.useState(false);
  const [recognizingJobId, setRecognizingJobId] = React.useState<number | null>(null);
  const [expandedJobId, setExpandedJobId] = React.useState<number | null>(null);
  const [result, setResult] = React.useState<ImportJobListResult | null>(null);
  const [page, setPage] = React.useState(1);
  const [recognizedFileIds, setRecognizedFileIds] = React.useState<Set<number>>(new Set());
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
      onError(String(err));
    }
  }, [onError]);

  React.useEffect(() => {
    fetchJobs(1);
  }, [refreshKey, fetchJobs]);

  const doImport = React.useCallback(
    async (paths: string[]) => {
      setIsImporting(true);
      try {
        await importFiles(paths);
        // Auto-recognition is triggered by backend after import
        // Poll for updated job list after a short delay
        setTimeout(() => fetchJobs(1), 1500);
      } catch (err) {
        onError(String(err));
      } finally {
        setIsImporting(false);
      }
    },
    [fetchJobs, onError],
  );

  const handlePickFiles = async () => {
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
  };

  const handleRecognize = async (job: ImportJob) => {
    if (!job.raw_file_id) {
      onError("该导入任务没有可识别的 RAW 文件。");
      return;
    }
    if (!llmApiKey.trim()) {
      onError("请先在设置中填写 LLM API Key。");
      return;
    }
    setRecognizingJobId(job.id);
    try {
      await recognizeRawFile({
        raw_file_id: job.raw_file_id,
        config: {
          base_url: llmBaseUrl,
          api_key: llmApiKey,
          model: llmModel,
          timeout_seconds: 90,
        },
      });
      onInvoicesAdded();
      setExpandedJobId(job.id);
    } catch (err) {
      onError(String(err));
    } finally {
      setRecognizingJobId(null);
    }
  };

  const allJobs = result?.jobs ?? [];
  const activeJobs = allJobs.filter((j) =>
    ["pending", "processing"].includes(j.status),
  );
  const completedJobs = allJobs.filter(
    (j) => !["pending", "processing"].includes(j.status),
  );

  return (
    <div className="page">
      <div className="page-header">
        <h2 className="page-title" style={{ margin: 0 }}>导入发票</h2>
        <div className="page-header-actions">
          <button className="btn-primary" onClick={handlePickFiles} disabled={isImporting}>
            {isImporting ? "导入中..." : "选择文件"}
          </button>
        </div>
      </div>

      <div className="import-zone">
        <div
          className={`drop-area ${isDraggingFiles ? "drop-area-active" : ""}`}
        >
          <span className="drop-icon">📎</span>
          <p>{isDraggingFiles ? "松开以导入文件" : "拖入 PDF / PNG / JPG / JPEG 文件"}</p>
          <span className="drop-hint">或点击右上角 "选择文件" 按钮</span>
        </div>
      </div>

      {activeJobs.length > 0 ? (
        <div className="section">
          <h3>进行中</h3>
          <div className="job-list">
            {activeJobs.map((job) => (
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
            <span className="badge">{result.total_count}</span>
          ) : null}
        </h3>
        {completedJobs.length === 0 ? (
          <p className="muted">暂无导入记录。</p>
        ) : (
          <>
            <div className="job-list">
              {completedJobs.map((job) => (
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
  return (
    <article
      className={`job-card ${expandedJobId === job.id ? "job-card-expanded" : ""}`}
      onClick={() =>
        onToggleExpand(expandedJobId === job.id ? null : job.id)
      }
    >
      <div className="job-card-header">
        <div className="job-card-info">
          <strong>{job.original_name ?? job.source_path}</strong>
          <span className="job-meta">
            {job.mime_type ? `${job.mime_type} · ` : ""}
            {job.status === "duplicate" ? "SHA256 重复" : importStatusMeta(job.status).label}
          </span>
        </div>
        <div className="job-card-actions" onClick={(e) => e.stopPropagation()}>
          <span className={`status-tag ${toneClass(importStatusMeta(job.status).tone)}`}>
            {importStatusMeta(job.status).label}
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
          <dl>
            <dt>源路径</dt>
            <dd>{job.source_path}</dd>
            {job.current_name ? (
              <>
                <dt>存储名</dt>
                <dd>{job.current_name}</dd>
              </>
            ) : null}
            {job.sha256 ? (
              <>
                <dt>SHA256</dt>
                <dd className="mono">{job.sha256.slice(0, 32)}...</dd>
              </>
            ) : null}
            {job.error_message ? (
              <>
                <dt>错误</dt>
                <dd className="text-error">{job.error_message}</dd>
              </>
            ) : null}
            <dt>时间</dt>
            <dd>{job.created_at}</dd>
          </dl>
        </div>
      ) : null}
    </article>
  );
}

function canRecognizeJob(job: ImportJob) {
  return (
    job.raw_file_id !== null &&
    job.status === "completed" &&
    (job.mime_type === "image/png" ||
      job.mime_type === "image/jpeg" ||
      job.mime_type === "application/pdf")
  );
}
