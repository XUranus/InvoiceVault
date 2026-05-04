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

const COLORS = [
  "var(--chart-series-1)",
  "var(--chart-series-2)",
  "var(--chart-series-3)",
  "var(--chart-series-4)",
  "var(--chart-series-5)",
  "var(--chart-series-6)",
  "var(--chart-series-7)",
  "var(--chart-series-8)",
];

const chartGrid = "var(--color-chart-grid)";
const chartAxis = "var(--color-chart-axis)";
const chartText = "var(--color-chart-tooltip-text)";
const chartPrimary = "var(--chart-series-1)";
const chartSecondary = "var(--chart-series-2)";
const chartBar = "var(--chart-bar-primary)";
const chartTooltip = {
  background: "var(--color-chart-tooltip-bg)",
  border: "1px solid var(--color-chart-tooltip-border)",
  borderRadius: 8,
  color: chartText,
  fontSize: 13,
};

type Props = {
  stats: DashboardStatsType;
};

export function DashboardStats({ stats }: Props) {
  return (
    <>
      {stats.total_invoices === 0 ? (
        <p className="dashboard-empty dashboard-chart-empty">
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
                  <CartesianGrid strokeDasharray="3 3" stroke={chartGrid} />
                  <XAxis dataKey="month" stroke={chartAxis} fontSize={12} />
                  <YAxis yAxisId="left" stroke={chartPrimary} fontSize={12} />
                  <YAxis yAxisId="right" orientation="right" stroke={chartSecondary} fontSize={12} />
                  <Tooltip
                    contentStyle={chartTooltip}
                    labelStyle={{ color: chartText }}
                    itemStyle={{ color: chartText }}
                  />
                  <Legend />
                  <Line
                    yAxisId="left"
                    type="monotone"
                    dataKey="count"
                    name="发票数"
                    stroke={chartPrimary}
                    strokeWidth={2}
                    dot={{ r: 3, fill: chartPrimary, stroke: chartPrimary }}
                    activeDot={{ r: 5, fill: chartPrimary, stroke: "var(--color-surface-elevated)", strokeWidth: 2 }}
                  />
                  <Line
                    yAxisId="right"
                    type="monotone"
                    dataKey="amount"
                    name="金额"
                    stroke={chartSecondary}
                    strokeWidth={2}
                    dot={{ r: 3, fill: chartSecondary, stroke: chartSecondary }}
                    activeDot={{ r: 5, fill: chartSecondary, stroke: "var(--color-surface-elevated)", strokeWidth: 2 }}
                  />
                </LineChart>
              </ResponsiveContainer>
            ) : (
              <p className="muted">暂无月度数据。</p>
            )}
          </div>

          {/* Type + Top sellers */}
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
                    labelLine={{ stroke: chartAxis }}
                  >
                    {stats.by_type.map((_, i) => (
                      <Cell key={i} fill={COLORS[i % COLORS.length]} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={chartTooltip}
                    labelStyle={{ color: chartText }}
                    itemStyle={{ color: chartText }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>

            <div className="chart-container">
              <h3 className="chart-title">Top 供应商（按发票数）</h3>
              <ResponsiveContainer width="100%" height={240}>
                <BarChart data={stats.top_sellers} layout="vertical">
                  <CartesianGrid strokeDasharray="3 3" stroke={chartGrid} />
                  <XAxis type="number" stroke={chartAxis} fontSize={12} />
                  <YAxis
                    type="category"
                    dataKey="seller_name"
                    stroke={chartAxis}
                    fontSize={12}
                    width={120}
                  />
                  <Tooltip
                    contentStyle={chartTooltip}
                    labelStyle={{ color: chartText }}
                    itemStyle={{ color: chartText }}
                  />
                  <Bar dataKey="count" name="发票数" fill={chartBar} radius={[0, 4, 4, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>
        </>
      )}
    </>
  );
}
