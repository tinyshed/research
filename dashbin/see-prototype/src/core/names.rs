//! Names as round 1c's `aq.py` reads them: tokens, a word's singular, and the
//! word lists that say what a column or a table may be.

use std::sync::LazyLock;

use regex::Regex;

static CAMEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-z0-9])([A-Z])").unwrap());
static CAPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([A-Z]+)([A-Z][a-z])").unwrap());
static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Za-z]+").unwrap());
static NORM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]").unwrap());

/// `orderDate`, `order_date`, `OrderDATE2` → ["order", "date"].
pub fn tokens(name: &str) -> Vec<String> {
    let s = CAMEL.replace_all(name, "$1 $2");
    let s = CAPS.replace_all(&s, "$1 $2");
    SPLIT.split(&s.to_lowercase()).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

pub fn norm(name: &str) -> String {
    NORM.replace_all(&name.to_lowercase(), "").into_owned()
}

pub fn singular(word: &str) -> String {
    let w = word.to_lowercase();
    for (a, b) in [("ies", "y"), ("sses", "ss"), ("ses", "s"), ("xes", "x"), ("s", "")] {
        if w.ends_with(a) && w.chars().count() > a.len() + 2 {
            return format!("{}{}", &w[..w.len() - a.len()], b);
        }
    }
    w
}

/// Whether a token, or its singular, is one of the words.
pub fn has(tk: &[String], words: &[&str]) -> bool {
    tk.iter().any(|t| words.contains(&t.as_str()) || words.contains(&singular(t).as_str()))
}

pub const CREATED: &[&str] = &["created", "create", "creation", "inserted", "registered", "signup", "joined", "opened", "added", "submitted", "posted"];
pub const UPDATED: &[&str] = &["updated", "modified", "changed", "edited", "touched", "synced", "refreshed", "seen", "login", "accessed"];
pub const DELETED: &[&str] = &["deleted", "removed", "archived"];
pub const BIRTH: &[&str] = &["birth", "dob", "birthday", "born", "death", "died", "expiry", "expires", "expiration", "valid", "until", "to", "end", "ends"];
pub const YEAR: &[&str] = &["year", "season", "yr"];
pub const YEARMONTH: &[&str] = &["yearmonth", "period", "month", "ym"];
pub const PLAN: &[&str] = &["scheduled", "planned", "expected", "estimated", "required", "due", "promised", "target", "ordered", "order", "requested", "start", "started", "begin", "opened", "created"];
pub const ACTUAL: &[&str] = &["actual", "shipped", "delivered", "completed", "closed", "resolved", "paid", "finished", "arrived", "done", "end", "ended", "received", "returned"];
pub const KEYISH: &[&str] = &["id", "uuid", "guid", "key", "hash", "token", "code", "sku", "ref", "phone", "fax", "zip", "postal", "postcode", "isbn", "ean", "upc", "iban", "ip", "url", "uri", "email", "mail", "password", "salt", "slug", "path", "file", "image", "photo", "picture", "avatar", "icon", "version", "lat", "lng", "lon", "latitude", "longitude", "number", "num", "no", "nr"];
pub const MONEY: &[&str] = &["amount", "price", "total", "subtotal", "cost", "costs", "fee", "fees", "charge", "charges", "tax", "paid", "salary", "wage", "wages", "revenue", "income", "profit", "freight", "budget", "spend", "spent", "payment", "balance", "debit", "credit", "value", "sales", "turnover", "margin", "earnings", "bonus", "commission", "rent", "fare", "tip", "refund", "deposit", "withdrawal", "premium", "money", "cash"];
pub const UNITS: &[&str] = &["qty", "quantity", "units", "count", "items", "sold", "pieces", "pcs", "tickets", "seats", "places", "nights", "number", "num", "amount", "volume", "liters", "litres", "kg"];
pub const QUANTITY: &[&str] = &["qty", "quantity", "count", "units", "items", "points", "wins", "losses", "goals", "laps", "votes", "views", "clicks", "visits", "hits", "likes", "downloads", "plays", "sold", "consumption", "distance", "minutes", "hours", "seconds", "duration", "milliseconds", "ms", "length", "volume", "pages", "steps", "calories", "kills", "deaths", "assists", "runs", "attendance", "seats", "capacity", "stock", "weight", "size", "bytes", "impressions", "conversions", "orders", "sessions"];
pub const RATIO: &[&str] = &["discount", "rate", "percent", "percentage", "pct", "ratio", "share", "probability", "avg", "average", "mean", "factor"];
pub const SCORE: &[&str] = &["rating", "score", "stars", "rank", "ranking", "priority", "position", "grid", "round", "level", "grade", "age", "lap"];
pub const CATEGORY: &[&str] = &["status", "state", "type", "kind", "category", "level", "role", "tier", "plan", "country", "source", "channel",
    "gender", "sex", "segment", "currency", "method", "mode", "class", "group", "league", "division", "genre",
    "language", "lang", "platform", "device", "browser", "os", "region", "city", "province", "nationality",
    "brand", "condition", "stage", "phase", "format", "medium", "unit", "size", "color", "colour", "pos",
    "bats", "throws", "hand", "conditions", "fare", "department", "dept", "team", "sport", "series", "result",
    "outcome", "reason", "gateway", "provider", "carrier", "vendor", "plan", "membership", "subscription"];
pub const GEO: &[&str] = &["country", "city", "state", "region", "province", "nationality", "continent", "county", "district", "location"];
pub const LABEL: &[&str] = &["name", "title", "label", "description", "desc", "caption", "subject", "headline", "heading", "nickname", "username", "login", "handle",
    "nome", "nombre", "nom", "titulo", "titel", "naam", "ten", "nazwa", "nazev"];
pub const PERSON_PART: &[&str] = &["first", "last", "given", "family", "middle", "full", "fore", "sur", "maiden", "nick"];
pub const PERSON_TABLE: &[&str] = &["user", "users", "customer", "customers", "employee", "employees", "member", "members", "person", "people",
    "persons", "player", "players", "driver", "drivers", "author", "authors", "student", "students", "teacher",
    "teachers", "patient", "patients", "client", "clients", "staff", "contact", "contacts", "account", "accounts",
    "manager", "managers", "agent", "agents", "actor", "actors", "artist", "artists", "doctor", "doctors"];
pub const LONG_TEXT: &[&str] = &["longtext", "mediumtext", "clob", "nclob", "ntext", "json", "jsonb", "xml", "bytea", "blob", "longblob", "mediumblob", "binary", "varbinary", "image"];
pub const SNAPSHOT: &[&str] = &["standing", "standings", "snapshot", "snapshots", "balance", "balances", "inventory", "inventories", "stock", "stocks", "history"];

pub static BOILERPLATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(django_|auth_|oauth|migrations?$|schema_migrations|ar_internal|failed_jobs|password_resets?|personal_access_tokens|sessions?$|cache|jobs$|job_batches|flyway|databasechangelog|sqlite_|qrtz_|act_|permissions?$|role_has|model_has|users_roles|spring_session|pg_stat)",
    )
    .unwrap()
});

/// A column's family by its type, as `aq.family`.
pub fn family(type_name: &str, base_type: &str) -> &'static str {
    let t = if base_type.is_empty() { type_name } else { base_type }.to_lowercase();
    let full = type_name.to_lowercase();
    if full.contains("[]") || t.starts_with('_') || t == "array" {
        return "other";
    }
    if LONG_TEXT.contains(&t.as_str()) || LONG_TEXT.contains(&full.as_str()) {
        return "long";
    }
    if ["bool", "boolean", "bit"].contains(&t.as_str()) || full.starts_with("tinyint(1)") {
        return "bool";
    }
    if t.contains("timestamp") || t.contains("datetime") || ["smalldatetime", "datetime2"].contains(&t.as_str()) {
        return "timestamp";
    }
    if t == "date" {
        return "date";
    }
    if ["time", "timetz", "interval", "year"].contains(&t.as_str()) {
        return if t == "year" { "int" } else { "other" };
    }
    if t.contains("int") || t.contains("serial") || ["number", "tech_id"].contains(&t.as_str()) {
        return "int";
    }
    if ["decimal", "numeric", "money", "smallmoney", "udecimal"].contains(&t.as_str()) {
        return "decimal";
    }
    if ["float", "double", "real", "double precision", "doubleprecision", "float4", "float8"].contains(&t.as_str()) {
        return "float";
    }
    if t.contains("char") || ["text", "enum", "string", "tinytext", "set", "citext", "name", "character", "uuid"].contains(&t.as_str()) {
        return if t == "uuid" { "uuid" } else { "text" };
    }
    "other"
}

static LEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((\d+)").unwrap());
static SCALE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(\d+\s*,\s*(\d+)\)").unwrap());

pub fn length(type_name: &str) -> Option<i64> {
    LEN.captures(type_name).and_then(|c| c[1].parse().ok())
}

pub fn scale(type_name: &str) -> Option<i64> {
    SCALE.captures(type_name).and_then(|c| c[1].parse().ok())
}
