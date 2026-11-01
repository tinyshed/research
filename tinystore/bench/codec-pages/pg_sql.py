"""Write the psql script that loads one dataset's COPY files into PostgreSQL.

    python3 -I pg_sql.py <dataset> > /work/pg/out/<dataset>.sql

Every `/work/pg/copy/<dataset>__*.tsv` is loaded twice, the body column's
storage EXTERNAL (out of line, never compressed) and EXTENDED (the default:
pglz first, then out of line), each into a fresh table
`(id integer primary key, body bytea)`. After a checkpoint it writes one line
of sizes a load to psql's output, each row's stored size and place to
`<name>.<storage>.rows` and each TOAST chunk's to `<name>.<storage>.chunks`.
"""

import glob
import os
import sys

SIZES = (
    "select '{name}', '{storage}', "
    "pg_relation_size('t','main'), pg_relation_size('t','fsm'), pg_relation_size('t','vm'), "
    "pg_relation_size(:'toast','main'), pg_relation_size(:'toast','fsm'), pg_relation_size(:'toast','vm'), "
    "pg_relation_size(:'toastidx'), pg_relation_size('t_pkey'), pg_total_relation_size('t'), "
    "(select count(distinct (ctid::text::point)[0]) from t), "
    "(select count(distinct (ctid::text::point)[0]) from :toast), "
    "(select count(*) from :toast), (select count(*) from t);"
)


def main():
    ds = sys.argv[1]
    bs = chr(92)  # a backslash, built so no tool turns it into anything else
    out = []
    for path in sorted(glob.glob(f"/work/pg/copy/{ds}__*.tsv")):
        name = os.path.basename(path)[:-4]
        for storage in ("external", "extended"):
            out += [
                "drop table if exists t;",
                "create table t (id integer primary key, body bytea);",
                f"alter table t alter column body set storage {storage};",
                f"copy t from '{path}';",
                "checkpoint;",
                "select reltoastrelid::regclass as toast, "
                "(select indexrelid::regclass from pg_index where indrelid = c.reltoastrelid) as toastidx "
                f"from pg_class c where oid = 't'::regclass {bs}gset",
                SIZES.format(name=name, storage=storage),
                "copy (select id, pg_column_size(body), coalesce(pg_column_compression(body), ''), "
                "pg_column_toast_chunk_id(body) is not null, (ctid::text::point)[0]::int from t order by id) "
                f"to '/work/pg/out/{name}.{storage}.rows';",
                "copy (select chunk_id, chunk_seq, length(chunk_data), (ctid::text::point)[0]::int "
                f"from :toast order by chunk_id, chunk_seq) to '/work/pg/out/{name}.{storage}.chunks';",
                # every page's header, so that each page's bytes can be checked against the tuples on it
                "copy (select 'heap', b, h.lower, h.upper, h.special from "
                "generate_series(0, pg_relation_size('t') / 8192 - 1) b, "
                "page_header(get_raw_page('t', b::int)) h union all "
                "select 'toast', b, h.lower, h.upper, h.special from "
                "generate_series(0, pg_relation_size(:'toast') / 8192 - 1) b, "
                "page_header(get_raw_page(:'toast', b::int)) h) "
                f"to '/work/pg/out/{name}.{storage}.pages';",
            ]
    out.insert(0, "create extension if not exists pageinspect;")
    out.append("drop table if exists t;")
    print("\n".join(out))


if __name__ == "__main__":
    main()
