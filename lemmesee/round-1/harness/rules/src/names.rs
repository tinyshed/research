//! Names and types as the rules read them, the same on both engines.

use model::Column;

/// A name's tokens, lower case, split on `_`, `-`, spaces, digits and case
/// changes: `InvoiceDate` and `invoice_date` both give `invoice`, `date`.
pub fn tokens(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphabetic() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        if c.is_uppercase() && !current.is_empty() {
            let previous = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            // `invoiceDate` splits before D; `HTTPServer` splits before S
            if previous.is_lowercase() || (previous.is_uppercase() && next_lower) {
                out.push(std::mem::take(&mut current));
            }
        }
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// A name's identity across engines: lower case, letters and digits only,
/// so that `invoice_line` and `InvoiceLine` are one table.
pub fn norm(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

pub fn any(tokens: &[String], words: &[&str]) -> bool {
    tokens.iter().any(|t| words.contains(&t.as_str()))
}

fn base(c: &Column) -> String {
    c.base_type.to_lowercase()
}

pub fn is_time(c: &Column) -> bool {
    let b = base(c);
    b.starts_with("timestamp") || b == "date" || b == "datetime"
}

pub fn is_bool(c: &Column) -> bool {
    let b = base(c);
    b == "boolean" || b == "bool" || c.type_name.to_lowercase() == "tinyint(1)"
}

pub fn is_integer(c: &Column) -> bool {
    let b = base(c);
    !is_bool(c)
        && matches!(
            b.as_str(),
            "smallint" | "integer" | "int" | "bigint" | "tinyint" | "mediumint" | "int2" | "int4" | "int8"
        )
}

pub fn is_decimal(c: &Column) -> bool {
    matches!(base(c).as_str(), "numeric" | "decimal" | "money")
}

pub fn is_float(c: &Column) -> bool {
    matches!(base(c).as_str(), "real" | "double precision" | "double" | "float" | "float4" | "float8")
}

pub fn is_numeric(c: &Column) -> bool {
    is_integer(c) || is_decimal(c) || is_float(c)
}

pub fn is_text(c: &Column) -> bool {
    matches!(
        base(c).as_str(),
        "text" | "character varying" | "varchar" | "character" | "char" | "bpchar" | "citext"
            | "tinytext" | "mediumtext" | "longtext" | "enum" | "set" | "nvarchar" | "nchar"
    )
}

pub fn is_uuid(c: &Column) -> bool {
    base(c) == "uuid"
}

/// The numbers in a type's parentheses: `numeric(10,2)` gives 10 and 2.
fn modifiers(c: &Column) -> Vec<u32> {
    let t = &c.type_name;
    let (Some(open), Some(close)) = (t.find('('), t.find(')')) else { return Vec::new() };
    t[open + 1..close].split(',').filter_map(|s| s.trim().parse().ok()).collect()
}

pub fn scale(c: &Column) -> Option<u32> {
    if base(c) == "money" {
        return Some(2);
    }
    modifiers(c).get(1).copied()
}

pub fn length(c: &Column) -> Option<u32> {
    if is_text(c) { modifiers(c).first().copied() } else { None }
}

/// Text too long to group by, by its type alone: MySQL's medium and long
/// texts, or a declared length over 1 000. PostgreSQL's `text` and the
/// `varchar(255)` many frameworks give every string say nothing: the
/// airlines' `flights.status` is `text`, ErgastF1's `status` `varchar(255)`.
pub fn long_text(c: &Column) -> bool {
    let b = base(c);
    matches!(b.as_str(), "mediumtext" | "longtext") || length(c).is_some_and(|n| n > 1000)
}

/// A default that stamps the moment a row is written.
pub fn stamps_now(c: &Column) -> bool {
    let d = c.default.as_deref().unwrap_or_default().to_lowercase();
    ["now()", "current_timestamp", "clock_timestamp", "localtimestamp", "statement_timestamp", "transaction_timestamp"]
        .iter()
        .any(|w| d.contains(w))
}

/// FNV-1a over a string, as a candidate's or a snapshot's identity.
pub fn hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_both_spellings_alike() {
        assert_eq!(tokens("invoice_date"), ["invoice", "date"]);
        assert_eq!(tokens("InvoiceDate"), ["invoice", "date"]);
        assert_eq!(tokens("SupportRepId"), ["support", "rep", "id"]);
        assert_eq!(tokens("HTTPServer2Port"), ["http", "server", "port"]);
        assert_eq!(norm("invoice_line"), norm("InvoiceLine"));
    }
}
