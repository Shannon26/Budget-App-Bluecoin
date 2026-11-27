use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row};

pub const UNCLASSIFIED_ACCOUNT_TYPE: &str = "unclassified";
pub const CREDIT_CARD_ACCOUNT_TYPE: &str = "credit_card";

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
    #[serde(default)]
    pub credit_limit_minor: Option<i64>,
    #[serde(default)]
    pub cutoff_day: Option<i64>,
    #[serde(default)]
    pub due_day: Option<i64>,
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
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub credit_limit_minor: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub cutoff_day: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub due_day: Option<Option<i64>>,
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i64>::deserialize(deserializer).map(Some)
}

fn default_currency() -> String {
    "INR".to_string()
}

#[derive(Debug, Clone)]
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
    pub credit_limit_minor: Option<i64>,
    pub cutoff_day: Option<i64>,
    pub due_day: Option<i64>,
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
            credit_limit_minor: row.try_get("credit_limit_minor")?,
            cutoff_day: row.try_get("cutoff_day")?,
            due_day: row.try_get("due_day")?,
            account_group: account_group_for(&account_type).to_string(),
            account_type,
        })
    }
}

#[derive(Debug, Clone)]
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
    pub credit_limit_minor: Option<i64>,
    pub cutoff_day: Option<i64>,
    pub due_day: Option<i64>,
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
            credit_limit_minor: row.try_get("credit_limit_minor")?,
            cutoff_day: row.try_get("cutoff_day")?,
            due_day: row.try_get("due_day")?,
            balance_minor: row.try_get("balance_minor")?,
            account_group: account_group_for(&account_type).to_string(),
            account_type,
        })
    }
}

fn visible_card_metadata(
    account_type: &str,
    credit_limit_minor: Option<i64>,
    cutoff_day: Option<i64>,
    due_day: Option<i64>,
) -> (Option<i64>, Option<i64>, Option<i64>) {
    if account_type == CREDIT_CARD_ACCOUNT_TYPE {
        (credit_limit_minor, cutoff_day, due_day)
    } else {
        (None, None, None)
    }
}

impl Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (credit_limit_minor, cutoff_day, due_day) = visible_card_metadata(
            &self.account_type,
            self.credit_limit_minor,
            self.cutoff_day,
            self.due_day,
        );
        let mut state = serializer.serialize_struct("Account", 14)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("currency", &self.currency)?;
        state.serialize_field("opening_balance_minor", &self.opening_balance_minor)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("note", &self.note)?;
        state.serialize_field("opening_date", &self.opening_date)?;
        state.serialize_field("hide_from_selection", &self.hide_from_selection)?;
        state.serialize_field("hide_from_reports", &self.hide_from_reports)?;
        state.serialize_field("account_type", &self.account_type)?;
        state.serialize_field("account_group", &self.account_group)?;
        state.serialize_field("credit_limit_minor", &credit_limit_minor)?;
        state.serialize_field("cutoff_day", &cutoff_day)?;
        state.serialize_field("due_day", &due_day)?;
        state.end()
    }
}

impl Serialize for AccountWithBalance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (credit_limit_minor, cutoff_day, due_day) = visible_card_metadata(
            &self.account_type,
            self.credit_limit_minor,
            self.cutoff_day,
            self.due_day,
        );
        let mut state = serializer.serialize_struct("AccountWithBalance", 15)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("currency", &self.currency)?;
        state.serialize_field("opening_balance_minor", &self.opening_balance_minor)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("note", &self.note)?;
        state.serialize_field("opening_date", &self.opening_date)?;
        state.serialize_field("hide_from_selection", &self.hide_from_selection)?;
        state.serialize_field("hide_from_reports", &self.hide_from_reports)?;
        state.serialize_field("balance_minor", &self.balance_minor)?;
        state.serialize_field("account_type", &self.account_type)?;
        state.serialize_field("account_group", &self.account_group)?;
        state.serialize_field("credit_limit_minor", &credit_limit_minor)?;
        state.serialize_field("cutoff_day", &cutoff_day)?;
        state.serialize_field("due_day", &due_day)?;
        state.end()
    }
}
