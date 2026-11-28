use serde::Serialize;

/// Exact decimal strings cross the API boundary; no floating-point conversion.
#[derive(Debug, Clone, Serialize)]
pub struct AssistantUsageDay {
    pub date: String,
    pub total_cost_usd: String,
    pub completed_count: i64,
    pub reported_count: i64,
    pub zero_cost_count: i64,
    pub missing_cost_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantUsageSummary {
    pub member_id: i64,
    pub total_cost_usd: String,
    pub completed_count: i64,
    pub reported_count: i64,
    pub zero_cost_count: i64,
    pub missing_cost_count: i64,
    pub days: Vec<AssistantUsageDay>,
}
