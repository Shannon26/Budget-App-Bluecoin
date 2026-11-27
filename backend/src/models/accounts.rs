use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row};

pub const UNCLASSIFIED_ACCOUNT_TYPE: &str = "unclassified";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AccountTypeDefinition {
    pub value: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

// This catalog is the single source of truth for account classification.
// The database stores only the type; the group is always derived from this table.
pub const ACCOUNT_TYPES: &[AccountTypeDefinition] = &[
    AccountTypeDefinition {
        value: UNCLASSIFIED_ACCOUNT_TYPE,
        label: "Unclassified",
        group: "unclassified",
    },
    AccountTypeDefinition {
        value: "cash",
        label: "Cash",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "checking",
        label: "Checking",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "savings",
        label: "Savings",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "wallet",
        label: "Wallet",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "investment",
        label: "Investment",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "cryptocurrency",
        label: "Cryptocurrency",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "property",
        label: "Property",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "receivable",
        label: "Receivable",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "other_asset",
        label: "Other Asset",
        group: "asset",
    },
    AccountTypeDefinition {
        value: "credit_card",
        label: "Credit Card",
        group: "liability",
    },
    AccountTypeDefinition {
        value: "loan",
        label: "Loan",
        group: "liability",
    },
    AccountTypeDefinition {
        value: "mortgage",
        label: "Mortgage",
        group: "liability",
    },
    AccountTypeDefinition {
        value: "payable",
        label: "Payable",
        group: "liability",
    },
    AccountTypeDefinition {
        value: "other_liability",
        label: "Other Liability",
        group: "liability",
    },
];

pub fn account_type_definition(value: &str) -> Option<&'static AccountTypeDefinition> {
    ACCOUNT_TYPES
        .iter()
        .find(|definition| definition.value == value)
}

pub fn account_group_for(value: &str) -> &'static str {
    account_type_definition(value).map_or("unclassified", |definition| definition.group)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub opening_balance_minor: i64,
    #[serde(default)]
    pub account_type: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub opening_date: Option<String>,
    #[serde(default)]
    pub hide_from_selection: bool,
    #[serde(default)]
    pub hide_from_reports: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAccountRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub note: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub opening_date: Option<Option<String>>,
    #[serde(default)]
    pub account_type: Option<String>,
    #[serde(default)]
    pub hide_from_selection: Option<bool>,
    #[serde(default)]
    pub hide_from_reports: Option<bool>,
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

fn default_currency() -> String {
    "INR".to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub opening_balance_minor: i64,
    pub created_at: String,
    pub note: String,
    pub opening_date: Option<String>,
    pub hide_from_selection: bool,
    pub hide_from_reports: bool,
    pub account_type: String,
    pub account_group: String,
}

impl FromRow<'_, SqliteRow> for Account {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let account_type: String = row.try_get("account_type")?;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            currency: row.try_get("currency")?,
            opening_balance_minor: row.try_get("opening_balance_minor")?,
            created_at: row.try_get("created_at")?,
            note: row.try_get("note")?,
            opening_date: row.try_get("opening_date")?,
            hide_from_selection: row.try_get("hide_from_selection")?,
            hide_from_reports: row.try_get("hide_from_reports")?,
            account_group: account_group_for(&account_type).to_string(),
            account_type,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountWithBalance {
    pub id: i64,
    pub name: String,
    pub currency: String,
    pub opening_balance_minor: i64,
    pub created_at: String,
    pub note: String,
    pub opening_date: Option<String>,
    pub hide_from_selection: bool,
    pub hide_from_reports: bool,
    pub balance_minor: i64,
    pub account_type: String,
    pub account_group: String,
}

impl FromRow<'_, SqliteRow> for AccountWithBalance {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let account_type: String = row.try_get("account_type")?;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            currency: row.try_get("currency")?,
            opening_balance_minor: row.try_get("opening_balance_minor")?,
            created_at: row.try_get("created_at")?,
            note: row.try_get("note")?,
            opening_date: row.try_get("opening_date")?,
            hide_from_selection: row.try_get("hide_from_selection")?,
            hide_from_reports: row.try_get("hide_from_reports")?,
            balance_minor: row.try_get("balance_minor")?,
            account_group: account_group_for(&account_type).to_string(),
            account_type,
        })
    }
}
