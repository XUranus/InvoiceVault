import type { AppHealth } from "../types";

type Props = {
  health: AppHealth | null;
  error: string | null;
  invoiceCount: number;
  jobCount: number;
};

export function DashboardPage({ health, error, invoiceCount, jobCount }: Props) {
  return (
    <div className="page">
      <h2 className="page-title">仪表盘</h2>

      {error ? (
        <div className="alert alert-error">{error}</div>
      ) : null}

      <div className="stat-cards">
        <div className="stat-card">
          <span className="stat-value">{invoiceCount}</span>
          <span className="stat-label">已入库发票</span>
        </div>
        <div className="stat-card">
          <span className="stat-value">{jobCount}</span>
          <span className="stat-label">导入任务</span>
        </div>
        <div className="stat-card">
          <span className="stat-value">
            {health ? "v" + health.migration_version : "-"}
          </span>
          <span className="stat-label">数据库版本</span>
        </div>
      </div>

      {health ? (
        <div className="section">
          <h3>系统信息</h3>
          <dl className="info-grid">
            <dt>数据目录</dt>
            <dd>{health.app_data_dir}</dd>
            <dt>数据库</dt>
            <dd>{health.database_path}</dd>
            <dt>迁移版本</dt>
            <dd>{health.migration_version}</dd>
          </dl>
        </div>
      ) : (
        <p className="muted">正在连接后端...</p>
      )}
    </div>
  );
}
