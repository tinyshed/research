"""What a living database says of itself that a dump does not: a default
of now() or ON UPDATE CURRENT_TIMESTAMP marks a technical time, a default
of 'pending' the start of a status's life, a generated column a line's
amount, a partial index's predicate a business rule a developer wrote, and
the tables' counts of inserts, updates and scans which tables are events,
which states and which lookups. Round 1's snapshot reads none of it.

    python -I intent.py <out>

Counts each kind of declaration in every database of rounds 1d-1f, on the
round's own PostgreSQL and MySQL; for a production database the same
queries go through round 1's harness, read only, and only counts leave it.
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))
import local  # noqa: E402

PG = {
    "default": "SELECT column_default FROM information_schema.columns WHERE table_schema NOT IN ('pg_catalog', "
               "'information_schema') AND column_default IS NOT NULL",
    "generated": "SELECT generation_expression FROM information_schema.columns WHERE table_schema NOT IN ('pg_catalog', "
                 "'information_schema') AND is_generated = 'ALWAYS'",
    "check": "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE contype = 'c' AND connamespace NOT IN "
             "('pg_catalog'::regnamespace, 'information_schema'::regnamespace)",
    "partial index": "SELECT pg_get_expr(indpred, indrelid) FROM pg_index i JOIN pg_class c ON c.oid = i.indrelid "
                     "WHERE indpred IS NOT NULL AND c.relnamespace NOT IN ('pg_catalog'::regnamespace)",
    "enum": "SELECT t.typname FROM pg_type t WHERE t.typtype = 'e'",
    "comment": "SELECT d.description FROM pg_description d JOIN pg_class c ON c.oid = d.objoid WHERE c.relnamespace "
               "NOT IN ('pg_catalog'::regnamespace, 'information_schema'::regnamespace)",
    "trigger": "SELECT action_statement FROM information_schema.triggers",
    "view": "SELECT table_name FROM information_schema.views WHERE table_schema NOT IN ('pg_catalog', 'information_schema')",
    "updated rows": "SELECT n_tup_upd FROM pg_stat_user_tables WHERE n_tup_upd > 0",
    "scanned index": "SELECT idx_scan FROM pg_stat_user_indexes WHERE idx_scan > 0",
}
MY = {
    "default": "SELECT COLUMN_DEFAULT FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() "
               "AND COLUMN_DEFAULT IS NOT NULL",
    "on update": "SELECT EXTRA FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND EXTRA LIKE '%on update%'",
    "generated": "SELECT GENERATION_EXPRESSION FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() "
                 "AND EXTRA LIKE '%GENERATED%' AND GENERATION_EXPRESSION <> ''",
    "check": "SELECT CHECK_CLAUSE FROM information_schema.CHECK_CONSTRAINTS WHERE CONSTRAINT_SCHEMA = DATABASE()",
    "enum": "SELECT COLUMN_TYPE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND DATA_TYPE IN ('enum', 'set')",
    "comment": "SELECT COLUMN_COMMENT FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND COLUMN_COMMENT <> ''",
    "trigger": "SELECT ACTION_STATEMENT FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA = DATABASE()",
    "view": "SELECT TABLE_NAME FROM information_schema.VIEWS WHERE TABLE_SCHEMA = DATABASE()",
}


def kind_of_default(v: str) -> str:
    v = (v or "").lower()
    if "nextval" in v or "auto_increment" in v:
        return "a key's sequence"
    if any(k in v for k in ("now()", "current_timestamp", "current_date", "localtimestamp")):
        return "the time of writing"
    if re.fullmatch(r"'?-?[\d.]+'?(::\w+.*)?", v):
        return "a number"
    if v in ("true", "false", "'t'", "'f'", "b'0'", "b'1'") or v.startswith(("true", "false")):
        return "a flag"
    if v.startswith("'") or re.fullmatch(r"[a-z_]+", v):
        return "a word"
    return "other"


def main(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    dg = local.module("intent_diagnose", ROOT / "round-1g" / "diagnose.py")
    see = dg.look1d.see
    corpus = ROOT / "corpus"
    totals, defaults, having, failed = Counter(), Counter(), Counter(), Counter()
    fes = sorted(p.parent.name for p in (corpus / "see").glob("*/snapshot.json"))
    for fe in fes:
        snap = json.loads((corpus / "see" / fe / "snapshot.json").read_text(encoding="utf-8"))
        engine, database = snap["engine"], snap["database"]
        for kind, sql in (MY if engine == "mysql" else PG).items():
            rows, err, _ = see.run_sql(engine, database, sql)
            if err:
                failed[kind] += 1
                continue
            totals[kind] += len(rows)
            having[kind] += bool(rows)
            if kind == "default":
                defaults.update(kind_of_default(r[0]) for r in rows)
    result = {"databases": len(fes), "declarations": dict(totals), "databases holding one": dict(having),
              "defaults by kind": dict(defaults), "queries failed": dict(failed)}
    out.mkdir(parents=True, exist_ok=True)
    (out / "intent.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
