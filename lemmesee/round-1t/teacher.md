# Round 1t's teacher prompt

Language models write schemas here: whole relational databases in a house
style, with every key declared, for the key model to learn naming styles it
has not seen. A teacher sees this prompt and nothing of ours: no schema of
any corpus, no candidate, no model's answer. Each request fills the three
slots below; `teach.py assign` draws the domains and styles with a fixed
seed and writes the filled prompts.

The styles were written on 8 November 2026, after round 1s's held-out
reading had been looked into, so LiveSQLBench's style is among what they
were written to cover; LiveSQLBench is development data from then on.

## The prompt

```
You are a database designer. Write {count} realistic relational database
schema(s), one for each assignment below, as a team working in the given
house style would have written it for a production system.

{assignments}

Rules for every schema:
- 6 to 25 tables. Include association (link) tables where the domain needs
  many-to-many relations.
- Every table declares a primary key: one column for most tables, several
  columns for link tables where natural.
- Declare every foreign key: the referencing table and column, and the
  referenced table and column. A referenced column is its table's primary
  key or a unique column.
- Most columns are ordinary attributes (names, descriptions, amounts,
  quantities, dates, statuses, flags, JSON documents), so that key columns
  are a minority, as in real databases.
- Use PostgreSQL types (integer, bigint, serial, uuid, text, varchar(n),
  numeric(p,s), date, timestamp, boolean, jsonb ...).
- Follow the house style in every table and column name, keys included.
- Do not explain anything. Output one JSON object and nothing else:

{"schemas": [{"domain": "...", "style": "...",
  "tables": [{"name": "...", "primary_key": ["..."],
              "columns": [{"name": "...", "type": "...", "nullable": true, "unique": false}]}],
  "foreign_keys": [{"table": "...", "column": "...", "ref_table": "...", "ref_column": "..."}]}]}
```

Each assignment is one line, `- domain: <domain>; house style: <style text>`.

## The styles

| Name | Text given to the teacher |
|---|---|
| classic | snake_case, plural table names, every primary key is named id, a reference is named after the referenced table in the singular plus _id |
| pascal | PascalCase singular table names; a primary key is the table name plus ID; a reference repeats the referenced key's name |
| abbreviated | terse abbreviations of 3 to 6 letters for words in table and column names (cust, ord, inv, qty, amt, dept); keys and references are built from the abbreviations with suffixes such as _no, _cd, _key or _ref |
| legacy upper | UPPER_SNAKE_CASE names of a decades-old enterprise system, cryptic and at most 10 characters (CUSTNO, ORDHDR, ITM_CD); a reference usually reuses or varies the referenced key's name |
| roles | references named by the role the referenced row plays rather than by its table (owner, approved_by, parent, assignee, origin_port, home_team), mixed with some conventional references |
| prefixed | Hungarian-style prefixes: tables start with tbl_ or t_, columns carry type or role prefixes such as fk_, pk_, n, s, d, dt (nCustomerId, sName, fk_order) |
| camel | lower-case table names, camelCase columns; references end in Ref, Link, Key or Fk at random (customerRef, orderLink, invoiceKey) |
| mixed | a schema grown over many years by several teams: each table follows a different convention (snake_case, camelCase, UPPER), references end in varying words (id, ref, link, key, fk, no, code) and some names are abbreviated |
| natural keys | text codes as primary keys where the domain has them (country_code, iso, sku, isbn, ticker, flight_no); references are named after the code they hold |
| foreign words | table and column names in transliterated German, Spanish, Portuguese or Russian words (kunde, auftrag, cliente, pedido, zakaz), keys in the same language's habits |
| handles | a primary key is the entity's word plus an unusual suffix (registry, handle, token, ident, serial, uid); a reference uses the same entity word with a different suffix (ptr, ref, link, tie, join) or a shortened form of it |
| warehouse | a data warehouse: dim_ and fact_ tables, surrogate keys ending in _sk or _key, fact tables referencing dimensions, some snowflaked dimensions referencing others |
