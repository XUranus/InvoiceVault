import {
  LineChart,
  Line,
  BarChart,
  Bar,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from "recharts";
import type { DashboardStats as DashboardStatsType } from "../types";

const COLORS = ["#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#ec4899", "#f97316"];

const STATUS_COLORS: Record<string, string> = {
  recognized: "#22c55e",
  pending_confirmation: "#f59e0b",
  needs_review: "#ef4444",
};

function formatAmount(value: number): string {
  if (value >= 10000) {
    return (value / 10000).toFixed(1) + " 万";
  }
  return value.toFixed(2);
}

type Props = {
  stats: DashboardStatsType;
};

export function DashboardStats({ stats }: Props) {
  return (
    <>
      {/* Stat cards */}
      <div className="stat-cards">
        <div className="stat-card">
          <span className="stat-value">{stats.total_invoices}</span>
          <span className="stat-label">已入库发票</span>
        </div>
        <div className="stat-card">
          <span className="stat-value">
            {stats.currency} {formatAmount(stats.total_amount)}
          </span>
          <span className="stat-label">金额合计</span>
        </div>
        <div className="stat-card">
          <span className="stat-value">{stats.this_month_count}</span>
          <span className="stat-label">本月新增</span>
        </div>
        <div className="stat-card">
          <span className="stat-value">{stats.pending_count}</span>
          <span className="stat-label">待确认</span>
        </div>
      </div>

      {stats.total_invoices === 0 ? (
        <p className="muted" style={{ marginTop: 24 }}>
          暂无数据。导入发票后这里会展示统计图表。
        </p>
      ) : (
        <>
          {/* Monthly trend */}
          <div className="chart-container">
            <h3 className="chart-title">月度趋势</h3>
            {stats.monthly_trend.length > 0 ? (
              <ResponsiveContainer width="100%" height={260}>
                <LineChart data={stats.monthly_trend}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#333" />
                  <XAxis dataKey="month" stroke="#999" fontSize={12} />
                  <YAxis yAxisId="left" stroke="#3b82f6" fontSize={12} />
                  <YAxis yAxisId="right" orientation="right" stroke="#22c55e" fontSize={12} />
                  <Tooltip
                    contentStyle={{
                      background: "#1e1e2e",
                      border: "1px solid #444",
                      borderRadius: 8,
                      fontSize: 13,
                    }}
                  />
                  <Legend />
                  <Line
                    yAxisId="left"
                    type="monotone"
                    dataKey="count"
                    name="发票数"
                    stroke="#3b82f6"
                    strokeWidth={2}
                    dot={{ r: 3 }}
                  />
                  <Line
                    yAxisId="right"
                    type="monotone"
                    dataKey="amount"
                    name="金额"
                    stroke="#22c55e"
                    strokeWidth={2}
                    dot={{ r: 3 }}
                  />
                </LineChart>
              </ResponsiveContainer>
            ) : (
              <p className="muted">暂无月度数据。</p>
            )}
          </div>

          {/* Type + Status */}
          <div className="chart-row-split">
            <div className="chart-container">
              <h3 className="chart-title">发票类型</h3>
              <ResponsiveContainer width="100%" height={240}>
                <PieChart>
                  <Pie
                    data={stats.by_type}
                    dataKey="count"
                    nameKey="label"
                    cx="50%"
                    cy="50%"
                    outerRadius={80}
                    label={({ name, value }) => `${name}: ${value}`}
                    labelLine={{ stroke: "#666" }}
                  >
                    {stats.by_type.map((_, i) => (
                      <Cell key={i} fill={COLORS[i % COLORS.length]} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{
                      background: "#1e1e2e",
                      border: "1px solid #444",
                      borderRadius: 8,
                      fontSize: 13,
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>

            <div className="chart-container">
              <h3 className="chart-title">状态分布</h3>
              <ResponsiveContainer width="100%" height={240}>
                <BarChart data={stats.by_status}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#333" />
                  <XAxis dataKey="label" stroke="#999" fontSize={12} />
                  <YAxis stroke="#999" fontSize={12} />
                  <Tooltip
                    contentStyle={{
                      background: "#1e1e2e",
                      border: "1px solid #444",
                      borderRadius: 8,
                      fontSize: 13,
                    }}
                  />
                  <Bar dataKey="count" name="数量" radius={[4, 4, 0, 0]}>
                    {stats.by_status.map((entry, i) => (
                      <Cell
                        key={i}
                        fill={STATUS_COLORS[entry.label] || COLORS[i % COLORS.length]}
                      />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>

          {/* Top sellers */}
          <div className="chart-container">
            <h3 className="chart-title">Top 供应商（按发票数）</h3>
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={stats.top_sellers} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="#333" />
                <XAxis type="number" stroke="#999" fontSize={12} />
                <YAxis
                  type="category"
                  dataKey="seller_name"
                  stroke="#999"
                  fontSize={12}
                  width={120}
                />
                <Tooltip
                  contentStyle={{
                    background: "#1e1e2e",
                    border: "1px solid #444",
                    borderRadius: 8,
                    fontSize: 13,
                  }}
                />
                <Bar dataKey="count" name="发票数" fill="#8b5cf6" radius={[0, 4, 4, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </>
      )}
    </>
  );
}
