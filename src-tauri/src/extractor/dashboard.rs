use rusqlite::Connection;
use serde::Serialize;

use super::ExtractorError;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardStats {
    pub total_invoices: i64,
    pub total_amount: f64,
    pub currency: String,
    pub average_confidence: f64,
    pub this_month_count: i64,
    pub this_month_amount: f64,
    pub pending_count: i64,
    pub duplicate_count: i64,
    pub monthly_trend: Vec<MonthlyTrendPoint>,
    pub by_type: Vec<BreakdownItem>,
    pub by_status: Vec<BreakdownItem>,
    pub top_sellers: Vec<TopSellerItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthlyTrendPoint {
    pub month: String,
    pub count: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BreakdownItem {
    pub label: String,
    pub count: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopSellerItem {
    pub seller_name: String,
    pub count: i64,
    pub amount: f64,
}

fn sum_amount() -> &'static str {
    "COALESCE(SUM(CAST(total_amount AS REAL)), 0.0)"
}

fn build_issue_date_filter(
    date_from: Option<&str>,
    date_to: Option<&str>,
    first_param_index: usize,
) -> (String, Vec<String>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();

    if let Some(from) = date_from {
        clauses.push(format!(
            "issue_date >= ?{}",
            first_param_index + params.len()
        ));
        params.push(from.to_owned());
    }
    if let Some(to) = date_to {
        clauses.push(format!(
            "issue_date <= ?{}",
            first_param_index + params.len()
        ));
        params.push(to.to_owned());
    }

    let clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" AND {}", clauses.join(" AND "))
    };
    (clause, params)
}

pub fn get_dashboard_stats(
    conn: &Connection,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<DashboardStats, ExtractorError> {
    let this_month = chrono::Local::now().format("%Y-%m-01").to_string();
    let (aggregate_where, aggregate_filter_params) = build_issue_date_filter(date_from, date_to, 2);
    let mut aggregate_params = vec![this_month.clone()];
    aggregate_params.extend(aggregate_filter_params);

    let (
        total_invoices,
        total_amount,
        average_confidence,
        this_month_count,
        this_month_amount,
        pending_count,
        duplicate_count,
    ): (i64, f64, f64, i64, f64, i64, i64) = conn.query_row(
        &format!(
            "SELECT COUNT(*), {sum}, COALESCE(AVG(confidence), 0.0),
                    COALESCE(SUM(CASE WHEN issue_date >= ?1 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN issue_date >= ?1 THEN CAST(total_amount AS REAL) ELSE 0 END), 0.0),
                    COALESCE(SUM(CASE WHEN status = 'pending_confirmation' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN duplicate_status IN ('possible_duplicate', 'probable_duplicate') THEN 1 ELSE 0 END), 0)
             FROM invoices WHERE 1=1{where}",
            sum = sum_amount(),
            where = aggregate_where
        ),
        rusqlite::params_from_iter(aggregate_params.iter()),
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        },
    )?;

    let currency = conn
        .query_row(
            "SELECT currency FROM invoices GROUP BY currency ORDER BY COUNT(*) DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "CNY".to_string());

    let (trend_where, trend_params) = build_issue_date_filter(date_from, date_to, 1);
    let mut trend_stmt = conn.prepare(&format!(
        "SELECT strftime('%Y-%m', issue_date) as month, COUNT(*), {}
            FROM invoices
            WHERE issue_date IS NOT NULL{}
            GROUP BY month
            ORDER BY month DESC
            LIMIT 12",
        sum_amount(),
        trend_where
    ))?;
    let trend_rows: Vec<(String, i64, f64)> = trend_stmt
        .query_map(rusqlite::params_from_iter(trend_params.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut monthly_trend: Vec<MonthlyTrendPoint> = trend_rows
        .into_iter()
        .map(|(month, count, amount)| MonthlyTrendPoint {
            month,
            count,
            amount,
        })
        .collect();
    monthly_trend.reverse();

    let (type_where, type_params) = build_issue_date_filter(date_from, date_to, 1);
    let mut type_stmt = conn.prepare(&format!(
        "SELECT COALESCE(invoice_type, '未知') as label, COUNT(*), {}
            FROM invoices
            WHERE 1=1{}
            GROUP BY invoice_type
            ORDER BY COUNT(*) DESC",
        sum_amount(),
        type_where
    ))?;
    let by_type: Vec<BreakdownItem> = type_stmt
        .query_map(rusqlite::params_from_iter(type_params.iter()), |row| {
            Ok(BreakdownItem {
                label: row.get(0)?,
                count: row.get(1)?,
                amount: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let (status_where, status_params) = build_issue_date_filter(date_from, date_to, 1);
    let mut status_stmt = conn.prepare(&format!(
        "SELECT status, COUNT(*), {}
            FROM invoices
            WHERE 1=1{}
            GROUP BY status
            ORDER BY COUNT(*) DESC",
        sum_amount(),
        status_where
    ))?;
    let by_status: Vec<BreakdownItem> = status_stmt
        .query_map(rusqlite::params_from_iter(status_params.iter()), |row| {
            Ok(BreakdownItem {
                label: row.get(0)?,
                count: row.get(1)?,
                amount: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let (seller_where, seller_params) = build_issue_date_filter(date_from, date_to, 1);
    let mut seller_stmt = conn.prepare(&format!(
        "SELECT COALESCE(seller_name, '未知') as name, COUNT(*), {}
            FROM invoices
            WHERE 1=1{}
            GROUP BY seller_name
            ORDER BY COUNT(*) DESC
            LIMIT 5",
        sum_amount(),
        seller_where
    ))?;
    let top_sellers: Vec<TopSellerItem> = seller_stmt
        .query_map(rusqlite::params_from_iter(seller_params.iter()), |row| {
            Ok(TopSellerItem {
                seller_name: row.get(0)?,
                count: row.get(1)?,
                amount: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DashboardStats {
        total_invoices,
        total_amount,
        currency,
        average_confidence,
        this_month_count,
        this_month_amount,
        pending_count,
        duplicate_count,
        monthly_trend,
        by_type,
        by_status,
        top_sellers,
    })
}
