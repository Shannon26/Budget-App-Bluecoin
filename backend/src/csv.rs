use crate::date::date_only;
use crate::models::LedgerEntry;

pub const LEDGER_CSV_HEADERS: [&str; 12] = [
    "Date",
    "Type",
    "Name",
    "Account",
    "Source Account",
    "Destination Account",
    "Category",
    "Subcategory",
    "Amount",
    "Currency",
    "Status",
    "Note",
];

pub fn ledger_csv(entries: &[LedgerEntry]) -> String {
    let mut output = String::new();
    write_row(&mut output, LEDGER_CSV_HEADERS);

    for entry in entries {
        // Legacy invalid timestamps are omitted rather than leaking arbitrary text into Date.
        let date = date_only(&entry.occurred_at).unwrap_or_default();
        let kind = match entry.kind.as_str() {
            "income" => "Income",
            "expense" => "Expense",
            "transfer" => "Transfer",
            _ => continue,
        };
        let is_transfer = kind == "Transfer";
        let (account, source_account, destination_account) = if is_transfer {
            (
                String::new(),
                spreadsheet_safe(&entry.account_name),
                spreadsheet_safe(
                    entry
                        .counterpart_account_name
                        .as_deref()
                        .unwrap_or_default(),
                ),
            )
        } else {
            (
                spreadsheet_safe(&entry.account_name),
                String::new(),
                String::new(),
            )
        };
        let (category, subcategory) = if is_transfer {
            (String::new(), String::new())
        } else if let Some(category_name) = entry.category_name.as_deref() {
            if let Some(parent_name) = entry.category_parent_name.as_deref() {
                (
                    spreadsheet_safe(parent_name),
                    spreadsheet_safe(category_name),
                )
            } else {
                (spreadsheet_safe(category_name), String::new())
            }
        } else {
            ("Uncategorized".to_string(), String::new())
        };
        let sign = if entry.amount_minor < 0 { "-" } else { "" };
        let absolute_amount = entry.amount_minor.unsigned_abs();
        let amount = format!(
            "{sign}{}.{:02}",
            absolute_amount / 100,
            absolute_amount % 100
        );
        let status = if entry.reconciled {
            "Reconciled"
        } else {
            "Unreconciled"
        };
        let fields = [
            date,
            kind.to_string(),
            spreadsheet_safe(&entry.name),
            account,
            source_account,
            destination_account,
            category,
            subcategory,
            amount,
            "INR".to_string(),
            status.to_string(),
            spreadsheet_safe(&entry.note),
        ];
        write_row(&mut output, fields.iter().map(String::as_str));
    }

    output
}

fn spreadsheet_safe(value: &str) -> String {
    let first_significant = value.chars().find(|character| {
        !character.is_whitespace() && !matches!(character, '\u{feff}' | '\u{200b}')
    });
    let starts_with_control = value
        .chars()
        .next()
        .is_some_and(|character| matches!(character, '\t' | '\r' | '\n'));
    if starts_with_control
        || first_significant.is_some_and(|character| matches!(character, '=' | '+' | '-' | '@'))
    {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn write_row<'a>(output: &mut String, fields: impl IntoIterator<Item = &'a str>) {
    for (index, field) in fields.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        if field.contains([',', '"', '\r', '\n']) {
            output.push('"');
            for character in field.chars() {
                if character == '"' {
                    output.push('"');
                }
                output.push(character);
            }
            output.push('"');
        } else {
            output.push_str(field);
        }
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreadsheet_formula_prefixes_survive_csv_quoting() {
        assert_eq!(spreadsheet_safe("=SUM(1,2)"), "'=SUM(1,2)");
        assert_eq!(spreadsheet_safe("  @note"), "'  @note");
        assert_eq!(spreadsheet_safe("\t-100"), "'\t-100");
        assert_eq!(spreadsheet_safe("+value"), "'+value");
        assert_eq!(spreadsheet_safe("ordinary text"), "ordinary text");
    }

    #[test]
    fn csv_quotes_commas_quotes_and_line_breaks() {
        let mut output = String::new();
        write_row(
            &mut output,
            ["plain", "comma,value", "quote\"value", "line\nvalue"],
        );
        assert_eq!(
            output,
            "plain,\"comma,value\",\"quote\"\"value\",\"line\nvalue\"\n"
        );
    }
}
