import React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportJob } from "../types";
import { importFiles, recognizeRawFile } from "../api";

type Props = {
  jobs: ImportJob[];
  isDraggingFiles: boolean;
  llmApiKey: string;
  llmBaseUrl: string;
  llmModel: string;
  onJobsChange: (imported: ImportJob[]) => void;
  onInvoicesAdded: () => void;
  onError: (error: string) => void;
};

export function ImportPage({
  jobs,
  isDraggingFiles,
  llmApiKey,
  llmBaseUrl,
  llmModel,
  onJobsChange,
  onInvoicesAdded,
  onError,
}: Props) {
  const [pathsText, setPathsText] = React.useState("");
  const [isImporting, setIsImporting] = React.useState(false);
  const [recognizingJobId, setRecognizingJobId] = React.useState<number | null>(null);
  const [expandedJobId, setExpandedJobId] = React.useState<number | null>(null);

  const doImport = React.useCallback(
    async (paths: string[]) => {
      setIsImporting(true);
      try {
        const imported = await importFiles(paths);
        onJobsChange(imported);
      } catch (err) {
        onError(String(err));
      } finally {
        setIsImporting(false);
      }
    },
    [onJobsChange, onError],
  );

  const handleImport = async () => {
    const paths = pathsText
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean);
    if (paths.length === 0) {
      onError("请输入至少一个文件路径。");
      return;
    }
    await doImport(paths);
    setPathsText("");
  };

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
      const result = await recognizeRawFile({
        raw_file_id: job.raw_file_id,
        config: {
          base_url: llmBaseUrl,
          api_key: llmApiKey,
          model: llmModel,
          timeout_seconds: 90,
        },
      });
      onInvoicesAdded();
      // Show recognition result briefly
      setExpandedJobId(job.id);
    } catch (err) {
      onError(String(err));
    } finally {
      setRecognizingJobId(null);
    }
  };

  // Separate active jobs (not completed/failed/duplicate) from history
  const activeJobs = jobs.filter((j) =>
    ["pending", "processing"].includes(j.status),
  );
  const completedJobs = jobs.filter(
    (j) => !["pending", "processing"].includes(j.status),
  );

  return (
    <div className="page">
      <h2 className="page-title">导入发票</h2>

      <div className="import-zone">
        <div
          className={`drop-area ${isDraggingFiles ? "drop-area-active" : ""}`}
        >
          <span className="drop-icon">📎</span>
          <p>{isDraggingFiles ? "松开以导入文件" : "拖入 PDF / PNG / JPG / JPEG 文件"}</p>
          <span className="drop-hint">或使用下方路径输入或文件选择器</span>
        </div>

        <div className="import-form-row">
          <textarea
            value={pathsText}
            onChange={(e) => setPathsText(e.target.value)}
            placeholder="每行输入一个文件路径，支持 PDF / PNG / JPG / JPEG"
            rows={4}
            className="import-textarea"
          />
        </div>

        <div className="import-buttons">
          <button className="btn-primary" onClick={handlePickFiles} disabled={isImporting}>
            选择文件
          </button>
          <button className="btn-secondary" onClick={handleImport} disabled={isImporting}>
            {isImporting ? "导入中..." : "导入路径"}
          </button>
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
              />
            ))}
          </div>
        </div>
      ) : null}

      <div className="section">
        <h3>
          导入历史
          {completedJobs.length > 0 ? (
            <span className="badge">{completedJobs.length}</span>
          ) : null}
        </h3>
        {completedJobs.length === 0 ? (
          <p className="muted">暂无导入记录。</p>
        ) : (
          <div className="job-list">
            {completedJobs.slice(0, 50).map((job) => (
              <JobRow
                key={job.id}
                job={job}
                recognizingJobId={recognizingJobId}
                expandedJobId={expandedJobId}
                onRecognize={handleRecognize}
                onToggleExpand={setExpandedJobId}
              />
            ))}
          </div>
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
}: {
  job: ImportJob;
  recognizingJobId: number | null;
  expandedJobId: number | null;
  onRecognize: (job: ImportJob) => void;
  onToggleExpand: (id: number | null) => void;
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
            {job.status === "duplicate" ? "SHA256 重复" : statusLabel(job.status)}
          </span>
        </div>
        <div className="job-card-actions" onClick={(e) => e.stopPropagation()}>
          <span className={`status-tag tag-${job.status}`}>
            {statusLabel(job.status)}
          </span>
          {canRecognizeJob(job) ? (
            <button
              className="btn-small"
              disabled={recognizingJobId !== null}
              onClick={() => onRecognize(job)}
            >
              {recognizingJobId === job.id ? "识别中..." : "识别"}
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

function statusLabel(status: string) {
  const labels: Record<string, string> = {
    pending: "等待中",
    processing: "处理中",
    completed: "已完成",
    duplicate: "重复",
    failed: "失败",
  };
  return labels[status] ?? status;
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
