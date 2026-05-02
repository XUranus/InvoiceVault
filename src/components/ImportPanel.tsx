import React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportJob } from "../types";
import { importFiles } from "../api";
import { importStatusMeta, toneClass } from "../status";

type Props = {
  jobs: ImportJob[];
  recognizingJobId: number | null;
  isDraggingFiles: boolean;
  onJobsChange: (jobs: ImportJob[]) => void;
  onRecognize: (job: ImportJob) => void;
  onError: (error: string) => void;
};

export function ImportPanel({
  jobs,
  recognizingJobId,
  isDraggingFiles,
  onJobsChange,
  onRecognize,
  onError,
}: Props) {
  const [pathsText, setPathsText] = React.useState("");
  const [isImporting, setIsImporting] = React.useState(false);

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
      .map((line) => line.trim())
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
        {
          name: "发票文件",
          extensions: ["pdf", "png", "jpg", "jpeg"],
        },
      ],
    });

    if (!selected) return;

    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) return;

    await doImport(paths);
  };

  return (
    <div
      className={`panel import-panel ${isDraggingFiles ? "import-panel-dragging" : ""}`}
    >
      <h2>导入队列</h2>
      <div className="drop-target">
        {isDraggingFiles ? "松开鼠标导入文件" : "拖入 PDF/PNG/JPG/JPEG 文件"}
      </div>
      <div className="import-form">
        <textarea
          value={pathsText}
          onChange={(event) => setPathsText(event.target.value)}
          placeholder="每行一个 PDF/PNG/JPG/JPEG 文件路径"
          rows={5}
        />
        <div className="import-actions">
          <button type="button" onClick={handlePickFiles} disabled={isImporting}>
            选择文件
          </button>
          <button type="button" onClick={handleImport} disabled={isImporting}>
            {isImporting ? "导入中" : "导入路径"}
          </button>
        </div>
      </div>
      <div className="job-list">
        {jobs.length === 0 ? (
          <p className="muted">暂无导入任务。</p>
        ) : (
          jobs.map((job) => (
            <article className="job-row" key={job.id}>
              <div className="job-main">
                <strong>{job.original_name ?? job.source_path}</strong>
                {job.current_name ? <small>存储为 {job.current_name}</small> : null}
                <span>{job.source_path}</span>
                {job.error_message ? <em>{job.error_message}</em> : null}
              </div>
              <div className="job-actions">
                <span className={`job-status ${toneClass(importStatusMeta(job.status).tone)}`}>
                  {importStatusMeta(job.status).label}
                </span>
                {canRecognizeJob(job) ? (
                  <button
                    className="small-button"
                    type="button"
                    onClick={() => onRecognize(job)}
                    disabled={recognizingJobId !== null}
                  >
                    {recognizingJobId === job.id ? "识别中" : "识别"}
                  </button>
                ) : null}
              </div>
            </article>
          ))
        )}
      </div>
    </div>
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
