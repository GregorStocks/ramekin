#!/usr/bin/env python3
"""Postgres vs SQLite benchmark for the recipe hot paths (doc/sqlite-pilot-analysis.md).

Usage:
    .venv/bin/python scripts/sqlite-pilot-bench.py data/pipeline-snapshots \
        postgresql://ramekin:ramekin@localhost:54321/<scratch_db> \
        <scratch.sqlite> <n_recipes>

Drops and recreates the recipe tables in <scratch_db>, so point it at a database
created for the benchmark, never the dev or test database.

Loads real pipeline-snapshot recipes, replicated to n_recipes current recipes
(3 versions each, one user), into a schema mirroring the recipe tables, then
times list / search / sync / concurrent-write workloads on both backends.
"""

import json
import os
import random
import sqlite3
import statistics
import sys
import threading
import time
import unicodedata
import uuid
from datetime import datetime, timedelta, timezone

import psycopg

SNAP, PG_DSN, SQLITE_PATH, N = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
VERSIONS = 3
TAGS = [
    "dinner",
    "dessert",
    "quick",
    "vegetarian",
    "baking",
    "soup",
    "make-ahead",
    "favorite",
]
USER = str(uuid.uuid4())
random.seed(1)


def unaccent(s):
    if s is None:
        return None
    return "".join(
        c for c in unicodedata.normalize("NFKD", s) if not unicodedata.combining(c)
    )


def load_snapshots():
    out = []
    for f in sorted(os.listdir(SNAP)):
        d = json.load(open(os.path.join(SNAP, f)))
        out.append(d)
    return out


def gen_rows(snaps):
    base = datetime(2024, 1, 1, tzinfo=timezone.utc)
    tag_ids = {t: str(uuid.uuid4()) for t in TAGS}
    recipes, versions, rvt = [], [], []
    xid = 1000
    for i in range(N):
        s = snaps[i % len(snaps)]
        rid = str(uuid.uuid4())
        created = base + timedelta(minutes=i * 7)
        vid = None
        for v in range(VERSIONS):
            vid = str(uuid.uuid4())
            xid += 1
            title = s["title"] + ("" if i < len(snaps) else f" #{i // len(snaps)}")
            versions.append(
                dict(
                    id=vid,
                    recipe_id=rid,
                    title=title,
                    description=s.get("description"),
                    ingredients=json.dumps(s.get("ingredients") or []),
                    instructions=s.get("instructions") or "",
                    source_name=s.get("source_name"),
                    photo_ids=[str(uuid.uuid4())] if i % 3 else [],
                    notes="Family favorite, crème fraîche on top"
                    if i % 17 == 0
                    else None,
                    rating=(i % 5) + 1 if i % 4 else None,
                    created_at=created + timedelta(days=v),
                    change_xid=xid,
                )
            )
            for t in random.sample(TAGS, 2):
                rvt.append((vid, tag_ids[t]))
        recipes.append(
            dict(id=rid, user_id=USER, current_version_id=vid, created_at=created)
        )
    return tag_ids, recipes, versions, rvt


PG_SCHEMA = """
DROP TABLE IF EXISTS recipe_version_tags, user_tags, recipe_versions, recipes,
    step_outputs, ingredient_name_resolutions, shopping_list_items CASCADE;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE OR REPLACE FUNCTION f_unaccent(text) RETURNS text LANGUAGE sql IMMUTABLE
    PARALLEL SAFE STRICT
AS $$ SELECT public.unaccent('public.unaccent', $1) $$;
CREATE TABLE recipes (id uuid PRIMARY KEY, user_id uuid NOT NULL, current_version_id
    uuid,
  created_at timestamptz NOT NULL, deleted_at timestamptz, deleted_xid bigint);
CREATE INDEX idx_recipes_user_active ON recipes(user_id) WHERE deleted_at IS NULL;
CREATE TABLE recipe_versions (id uuid PRIMARY KEY, recipe_id uuid NOT NULL REFERENCES
    recipes(id),
  title varchar NOT NULL, description text, ingredients jsonb NOT NULL, instructions
    text NOT NULL,
  source_name varchar, photo_ids uuid[] NOT NULL, notes text, rating int, created_at
    timestamptz NOT NULL,
  change_xid bigint NOT NULL);
CREATE INDEX ON recipe_versions(recipe_id);
CREATE TABLE user_tags (id uuid PRIMARY KEY, user_id uuid NOT NULL, name text NOT NULL,
  deleted_at timestamptz, change_xid bigint NOT NULL DEFAULT 0);
CREATE TABLE recipe_version_tags (recipe_version_id uuid REFERENCES recipe_versions(id),
  tag_id uuid REFERENCES user_tags(id), PRIMARY KEY (recipe_version_id, tag_id));
CREATE TABLE step_outputs (id uuid PRIMARY KEY, scrape_job_id uuid NOT NULL,
  step_name text NOT NULL, output jsonb NOT NULL, created_at timestamptz NOT NULL);
CREATE INDEX ON step_outputs(scrape_job_id);
CREATE TABLE ingredient_name_resolutions (name text PRIMARY KEY, status text NOT NULL,
  result jsonb, updated_at timestamptz NOT NULL);
CREATE TABLE shopping_list_items (id uuid PRIMARY KEY, user_id uuid NOT NULL,
  item text NOT NULL, checked boolean NOT NULL, change_xid bigint NOT NULL,
  updated_at timestamptz NOT NULL);
CREATE INDEX ON shopping_list_items(user_id, change_xid);
"""
PG_INDEXES = """
CREATE INDEX ON recipe_versions USING GIN (f_unaccent(title) gin_trgm_ops);
CREATE INDEX ON recipe_versions USING GIN (f_unaccent(description) gin_trgm_ops);
CREATE INDEX ON recipe_versions USING GIN (f_unaccent(instructions) gin_trgm_ops);
CREATE INDEX ON recipe_versions USING GIN (f_unaccent(notes) gin_trgm_ops);
CREATE INDEX ON recipe_versions USING GIN (f_unaccent(ingredients::text) gin_trgm_ops);
ANALYZE;
"""

# remove_diacritics stands in for normalize_for_search here: same index cost, but
# it misses the contract's ligature expansions (see doc/sqlite-pilot-analysis.md).
SQLITE_SCHEMA = """
CREATE TABLE recipes (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, current_version_id
    TEXT,
  created_at TEXT NOT NULL, deleted_at TEXT, deleted_xid INTEGER);
CREATE INDEX idx_recipes_user_active ON recipes(user_id) WHERE deleted_at IS NULL;
CREATE TABLE recipe_versions (seq INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE,
  recipe_id TEXT NOT NULL REFERENCES recipes(id),
  title TEXT NOT NULL, description TEXT, ingredients TEXT NOT NULL, instructions TEXT
    NOT NULL,
  source_name TEXT, photo_ids TEXT NOT NULL, notes TEXT, rating INTEGER, created_at
    TEXT NOT NULL,
  change_xid INTEGER NOT NULL);
CREATE INDEX rv_recipe ON recipe_versions(recipe_id);
CREATE TABLE user_tags (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL,
  deleted_at TEXT, change_xid INTEGER NOT NULL DEFAULT 0);
CREATE TABLE recipe_version_tags (recipe_version_id TEXT REFERENCES recipe_versions(id),
  tag_id TEXT REFERENCES user_tags(id), PRIMARY KEY (recipe_version_id, tag_id))
    WITHOUT ROWID;
CREATE TABLE step_outputs (id TEXT PRIMARY KEY, scrape_job_id TEXT NOT NULL,
  step_name TEXT NOT NULL, output TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE INDEX so_job ON step_outputs(scrape_job_id);
CREATE TABLE ingredient_name_resolutions (name TEXT PRIMARY KEY, status TEXT NOT NULL,
  result TEXT, updated_at TEXT NOT NULL);
CREATE TABLE shopping_list_items (id TEXT PRIMARY KEY, user_id TEXT NOT NULL,
  item TEXT NOT NULL, checked INTEGER NOT NULL, change_xid INTEGER NOT NULL,
  updated_at TEXT NOT NULL);
CREATE INDEX sli_user ON shopping_list_items(user_id, change_xid);
CREATE TABLE sync_clock (id INTEGER PRIMARY KEY CHECK (id = 1), value INTEGER NOT NULL);
INSERT INTO sync_clock VALUES (1, 1000000);
CREATE VIRTUAL TABLE rv_fts USING fts5(title, description, instructions, notes,
    ingredients,
  content='', contentless_delete=1, tokenize='trigram remove_diacritics 1');
"""


def setup_pg(data):
    tag_ids, recipes, versions, rvt = data
    with psycopg.connect(PG_DSN, autocommit=True) as c:
        c.execute(PG_SCHEMA)
        with c.cursor() as cur:
            with cur.copy("COPY user_tags (id, user_id, name) FROM STDIN") as cp:
                for t, i in tag_ids.items():
                    cp.write_row((i, USER, t))
            with cur.copy(
                "COPY recipes (id, user_id, current_version_id, created_at) FROM STDIN"
            ) as cp:
                for r in recipes:
                    cp.write_row(
                        (
                            r["id"],
                            r["user_id"],
                            r["current_version_id"],
                            r["created_at"],
                        )
                    )
            with cur.copy(
                "COPY recipe_versions (id, recipe_id, title, description, "
                "ingredients, instructions,"
                " source_name, photo_ids, notes, rating, created_at, change_xid) FROM "
                "STDIN"
            ) as cp:
                for v in versions:
                    cp.write_row(
                        (
                            v["id"],
                            v["recipe_id"],
                            v["title"],
                            v["description"],
                            v["ingredients"],
                            v["instructions"],
                            v["source_name"],
                            v["photo_ids"],
                            v["notes"],
                            v["rating"],
                            v["created_at"],
                            v["change_xid"],
                        )
                    )
            with cur.copy("COPY recipe_version_tags FROM STDIN") as cp:
                for row in rvt:
                    cp.write_row(row)
        c.execute(PG_INDEXES)


def sqlite_connect():
    c = sqlite3.connect(
        SQLITE_PATH, timeout=30, isolation_level=None, check_same_thread=False
    )
    c.execute("PRAGMA journal_mode=WAL")
    c.execute("PRAGMA synchronous=FULL")
    c.execute("PRAGMA foreign_keys=ON")
    c.create_function("f_unaccent", 1, unaccent, deterministic=True)
    # ILIKE equivalent: SQLite LIKE is only ASCII-case-insensitive, so fold both sides.
    c.create_function(
        "fold",
        1,
        lambda s: unaccent(s).lower() if s is not None else None,
        deterministic=True,
    )
    return c


def setup_sqlite(data):
    tag_ids, recipes, versions, rvt = data
    for suffix in ("", "-wal", "-shm"):
        if os.path.exists(SQLITE_PATH + suffix):
            os.remove(SQLITE_PATH + suffix)
    c = sqlite_connect()
    c.executescript(SQLITE_SCHEMA)
    c.execute("BEGIN")
    c.executemany(
        "INSERT INTO user_tags (id, user_id, name) VALUES (?,?,?)",
        [(i, USER, t) for t, i in tag_ids.items()],
    )
    c.executemany(
        "INSERT INTO recipes (id, user_id, current_version_id, created_at) VALUES "
        "(?,?,?,?)",
        [
            (
                r["id"],
                r["user_id"],
                r["current_version_id"],
                r["created_at"].isoformat(),
            )
            for r in recipes
        ],
    )
    c.executemany(
        "INSERT INTO recipe_versions (id, recipe_id, title, description, ingredients, "
        "instructions, source_name, photo_ids, notes, rating, created_at, change_xid) "
        "VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        [
            (
                v["id"],
                v["recipe_id"],
                v["title"],
                v["description"],
                v["ingredients"],
                v["instructions"],
                v["source_name"],
                json.dumps(v["photo_ids"]),
                v["notes"],
                v["rating"],
                v["created_at"].isoformat(),
                v["change_xid"],
            )
            for v in versions
        ],
    )
    c.executemany("INSERT INTO recipe_version_tags VALUES (?,?)", rvt)
    c.execute(
        "INSERT INTO rv_fts (rowid, title, description, instructions, notes, "
        "ingredients) "
        "SELECT v.seq, v.title, v.description, v.instructions, v.notes, "
        "v.ingredients "
        "FROM recipe_versions v JOIN recipes r ON r.current_version_id = v.id"
    )
    c.execute("COMMIT")
    c.execute("ANALYZE")
    return c


# --- Queries --------------------------------------------------------------

PG_TAGS = (
    "(SELECT COALESCE(array_agg(ut.name ORDER BY ut.name), ARRAY[]::text[]) FROM "
    "recipe_version_tags rvt "
    "JOIN user_tags ut ON ut.id = rvt.tag_id WHERE rvt.recipe_version_id = v.id AND "
    "ut.deleted_at IS NULL)"
)
SL_TAGS = (
    "(SELECT COALESCE(json_group_array(name), '[]') FROM (SELECT ut.name FROM "
    "recipe_version_tags rvt "
    "JOIN user_tags ut ON ut.id = rvt.tag_id WHERE rvt.recipe_version_id = v.id AND "
    "ut.deleted_at IS NULL "
    "ORDER BY ut.name))"
)
CUR = (
    "FROM recipes r JOIN recipe_versions v ON v.id = r.current_version_id "
    "WHERE r.user_id = {p} AND r.deleted_at IS NULL"
)


def pg_list(c):
    c.execute(
        "SELECT p.rid::text, v.title, v.description, v.photo_ids::text, v.rating, "
        f"{PG_TAGS} FROM "
        f"(SELECT r.id AS rid, v.id AS vid, v.created_at AS ca {CUR.format(p='%s')} "
        "ORDER BY v.created_at DESC, r.id LIMIT 50) p JOIN recipe_versions v ON v.id "
        "= p.vid "
        "ORDER BY p.ca DESC, p.rid",
        (USER,),
    ).fetchall()
    c.execute(f"SELECT count(*) {CUR.format(p='%s')}", (USER,)).fetchone()


def sl_list(c):
    c.execute(
        f"SELECT p.rid, v.title, v.description, v.photo_ids, v.rating, {SL_TAGS} FROM "
        f"(SELECT r.id AS rid, v.id AS vid, v.created_at AS ca {CUR.format(p='?')} "
        "ORDER BY v.created_at DESC, r.id LIMIT 50) p JOIN recipe_versions v ON v.id "
        "= p.vid "
        "ORDER BY p.ca DESC, p.rid",
        (USER,),
    ).fetchall()
    c.execute(f"SELECT count(*) {CUR.format(p='?')}", (USER,)).fetchone()


FIELDS_PG = [
    "v.title",
    "v.description",
    "v.instructions",
    "v.notes",
    "v.ingredients::text",
]
FIELDS_SL = ["v.title", "v.description", "v.instructions", "v.notes", "v.ingredients"]


def pg_search(c, tokens):
    where = " AND ".join(
        "(" + " OR ".join(f"f_unaccent({f}) ILIKE %s" for f in FIELDS_PG) + ")"
        for _ in tokens
    )
    params = [USER] + [f"%{unaccent(t)}%" for t in tokens for _ in FIELDS_PG]
    return c.execute(
        "SELECT r.id::text, v.title, v.description, v.ingredients::text, "
        f"v.instructions, v.notes, {PG_TAGS} "
        f"{CUR.format(p='%s')} AND {where}",
        params,
    ).fetchall()


def sl_search_scan(c, tokens):
    where = " AND ".join(
        "(" + " OR ".join(f"fold({f}) LIKE ?" for f in FIELDS_SL) + ")" for _ in tokens
    )
    params = [USER] + [f"%{unaccent(t).lower()}%" for t in tokens for _ in FIELDS_SL]
    return c.execute(
        "SELECT r.id, v.title, v.description, v.ingredients, v.instructions, "
        f"v.notes, {SL_TAGS} "
        f"{CUR.format(p='?')} AND {where}",
        params,
    ).fetchall()


def sl_search_fts(c, tokens):
    # Trigram FTS5 matches substrings case- and diacritic-insensitively, for
    # tokens of 3+ characters. The unary + keeps the planner from probing the
    # recipe_versions index once per (recipe, match) pair, which is ~50x slower.
    match = " AND ".join('"' + unaccent(t).replace('"', '""') + '"' for t in tokens)
    return c.execute(
        "SELECT r.id, v.title, v.description, v.ingredients, v.instructions, "
        f"v.notes, {SL_TAGS} "
        f"{CUR.format(p='?')} AND +v.seq IN (SELECT rowid FROM rv_fts WHERE rv_fts "
        "MATCH ?)",
        (USER, match),
    ).fetchall()


def pg_sync_sweep(c, page=200):
    after, n = "00000000-0000-0000-0000-000000000000", 0
    with c.transaction():
        c.execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        while True:
            rows = c.execute(
                "SELECT r.id::text, v.id::text, v.title, v.description, "
                "v.ingredients::text, v.instructions, "
                "v.source_name, v.photo_ids::text, v.notes, v.rating, "
                "v.created_at::text, v.change_xid, "
                f"{PG_TAGS} {CUR.format(p='%s')} "
                "AND r.id > %s ORDER BY r.id LIMIT %s",
                (USER, after, page),
            ).fetchall()
            n += len(rows)
            if len(rows) < page:
                return n
            after = rows[-1][0]


def sl_sync_sweep(c, page=200):
    after, n = "", 0
    c.execute("BEGIN")
    try:
        while True:
            rows = c.execute(
                f"SELECT r.id, v.*, {SL_TAGS} {CUR.format(p='?')} AND r.id > ? ORDER "
                "BY r.id LIMIT ?",
                (USER, after, page),
            ).fetchall()
            n += len(rows)
            if len(rows) < page:
                return n
            after = rows[-1][0]
    finally:
        c.execute("COMMIT")


def timeit(fn, reps):
    ts = []
    for _ in range(reps):
        t = time.perf_counter()
        fn()
        ts.append((time.perf_counter() - t) * 1000)
    ts.sort()
    return statistics.median(ts), ts[int(len(ts) * 0.95) - 1] if len(ts) >= 20 else ts[
        -1
    ]


def fmt(t):
    return f"p50 {t[0]:7.2f} ms  p95 {t[1]:7.2f} ms"


# --- Writes ---------------------------------------------------------------


def pg_save(c, rid):
    with c.transaction():
        c.execute("SELECT 1 FROM recipes WHERE id = %s FOR UPDATE", (rid,))
        vid = str(uuid.uuid4())
        c.execute(
            "INSERT INTO recipe_versions (id, recipe_id, title, description, "
            "ingredients, instructions, "
            "source_name, photo_ids, notes, rating, created_at, change_xid) SELECT "
            "%s, recipe_id, title, "
            "description, ingredients, instructions, source_name, photo_ids, notes, "
            "rating, now(), "
            "pg_current_xact_id()::text::bigint FROM recipe_versions v WHERE v.id = "
            "(SELECT current_version_id FROM recipes WHERE id = %s)",
            (vid, rid),
        )
        c.execute(
            "INSERT INTO recipe_version_tags SELECT %s, tag_id FROM "
            "recipe_version_tags rvt "
            "WHERE rvt.recipe_version_id = (SELECT current_version_id FROM recipes "
            "WHERE id = %s)",
            (vid, rid),
        )
        c.execute(
            "UPDATE recipes SET current_version_id = %s WHERE id = %s", (vid, rid)
        )


def sl_save(c, rid):
    c.execute("BEGIN IMMEDIATE")
    try:
        vid = str(uuid.uuid4())
        # Single writer, so a counter bumped in the write transaction can replace
        # pg_current_xact_id.
        c.execute("UPDATE sync_clock SET value = value + 1")
        cur = c.execute(
            "SELECT current_version_id FROM recipes WHERE id = ?", (rid,)
        ).fetchone()[0]
        c.execute(
            "INSERT INTO recipe_versions (id, recipe_id, title, description, "
            "ingredients, instructions, "
            "source_name, photo_ids, notes, rating, created_at, change_xid) SELECT ?, "
            "recipe_id, title, "
            "description, ingredients, instructions, source_name, photo_ids, notes, "
            "rating, "
            "strftime('%Y-%m-%dT%H:%M:%fZ'), (SELECT value FROM sync_clock) "
            "FROM recipe_versions v WHERE v.id = ?",
            (vid, cur),
        )
        c.execute(
            "INSERT INTO recipe_version_tags SELECT ?, tag_id FROM "
            "recipe_version_tags WHERE recipe_version_id = ?",
            (vid, cur),
        )
        c.execute("UPDATE recipes SET current_version_id = ? WHERE id = ?", (vid, rid))
        old_rowid = c.execute(
            "SELECT seq FROM recipe_versions WHERE id = ?", (cur,)
        ).fetchone()[0]
        c.execute("DELETE FROM rv_fts WHERE rowid = ?", (old_rowid,))
        c.execute(
            "INSERT INTO rv_fts (rowid, title, description, instructions, notes, "
            "ingredients) "
            "SELECT seq, title, description, instructions, notes, ingredients FROM "
            "recipe_versions WHERE id = ?",
            (vid,),
        )
        c.execute("COMMIT")
    except Exception:
        c.execute("ROLLBACK")
        raise


def pg_bulk_rewrite(c):
    with c.transaction():
        c.execute(
            "UPDATE recipe_versions v SET ingredients = v.ingredients FROM recipes r "
            "WHERE r.current_version_id = v.id"
        )


def sl_bulk_rewrite(c):
    # The FTS index is refreshed in the same transaction, as the port must do;
    # Postgres's UPDATE maintains its GIN indexes implicitly.
    current = (
        "SELECT v.seq FROM recipe_versions v "
        "JOIN recipes r ON r.current_version_id = v.id"
    )
    c.execute("BEGIN IMMEDIATE")
    c.execute(
        "UPDATE recipe_versions SET ingredients = json(ingredients) "
        "WHERE id IN (SELECT current_version_id FROM recipes)"
    )
    c.execute(f"DELETE FROM rv_fts WHERE rowid IN ({current})")
    c.execute(
        "INSERT INTO rv_fts (rowid, title, description, instructions, notes, "
        "ingredients) SELECT seq, title, description, instructions, notes, "
        f"ingredients FROM recipe_versions WHERE seq IN ({current})"
    )
    c.execute("COMMIT")


def concurrent(connect, read_fns, save, rids, seconds=5.0, readers=4):
    stop = time.perf_counter() + seconds
    reads, writes, errors = [], [], []

    def reader():
        c = connect()
        while time.perf_counter() < stop:
            fn = random.choice(read_fns)
            t = time.perf_counter()
            fn(c)
            reads.append((time.perf_counter() - t) * 1000)

    def writer():
        c = connect()
        while time.perf_counter() < stop:
            t = time.perf_counter()
            try:
                save(c, random.choice(rids))
            except Exception as e:  # noqa: BLE001
                errors.append(repr(e))
            writes.append((time.perf_counter() - t) * 1000)

    ths = [threading.Thread(target=reader) for _ in range(readers)] + [
        threading.Thread(target=writer) for _ in range(2)
    ]
    for t in ths:
        t.start()
    for t in ths:
        t.join()
    reads.sort()
    writes.sort()
    q = lambda xs, p: xs[min(len(xs) - 1, int(len(xs) * p))]  # noqa: E731
    return (
        f"reads n={len(reads)} p50 {q(reads, 0.5):.2f} p99 {q(reads, 0.99):.2f} ms | "
        f"writes n={len(writes)} p50 {q(writes, 0.5):.2f} p99 {q(writes, 0.99):.2f} "
        f"ms | errors {len(errors)}" + (f" {errors[0]}" if errors else "")
    )


WRITE_LOCK = threading.Lock()


def sl_save_serialized(c, rid):
    with WRITE_LOCK:
        sl_save(c, rid)


# --- Non-recipe tables and the mixed background workload ------------------
# Sizes are scaled from the test database: step outputs average ~6 KB
# compressed in Postgres with fetched HTML up to ~130 KB. SQLite doesn't
# compress, so raw sizes are used: 9 small JSON outputs plus one ~100 KB HTML
# blob per scrape job, one job per recipe, plus 10k ingredient
# names and 2k shopping-list items.
STEPS_PER_JOB = 10
N_NAMES = 10_000
N_SHOPPING = 2_000


def step_payload(i):
    if i % STEPS_PER_JOB == 0:
        html = "<p>" + ("Lorem ipsum dolor sit amet, consectetur adipiscing. " * 1900)
        return json.dumps({"html": html})
    return json.dumps(
        {"step": i, "items": [{"k": j, "v": "x" * 30} for j in range(60)]}
    )


def extra_rows(n_recipes):
    jobs = [str(uuid.uuid4()) for _ in range(n_recipes)]
    now = datetime.now(timezone.utc)
    steps = [
        (
            str(uuid.uuid4()),
            jobs[i // STEPS_PER_JOB],
            f"step{i % STEPS_PER_JOB}",
            i,
            now,
        )
        for i in range(n_recipes * STEPS_PER_JOB)
    ]
    names = [(f"ingredient name {i}", "resolved", now) for i in range(N_NAMES)]
    shopping = [
        (str(uuid.uuid4()), USER, f"item {i}", i % 2 == 0, 1000 + i, now)
        for i in range(N_SHOPPING)
    ]
    return jobs, steps, names, shopping


def seed_extra_pg(extra):
    _, steps, names, shopping = extra
    with psycopg.connect(PG_DSN, autocommit=True) as c, c.cursor() as cur:
        with cur.copy(
            "COPY step_outputs (id, scrape_job_id, step_name, output, created_at) "
            "FROM STDIN"
        ) as cp:
            for sid, job, step, i, ts in steps:
                cp.write_row((sid, job, step, step_payload(i), ts))
        with cur.copy(
            "COPY ingredient_name_resolutions (name, status, result, updated_at) "
            "FROM STDIN"
        ) as cp:
            for name, status, ts in names:
                cp.write_row((name, status, '{"grams": 1}', ts))
        with cur.copy("COPY shopping_list_items FROM STDIN") as cp:
            for row in shopping:
                cp.write_row(row)
        c.execute("ANALYZE")


def seed_extra_sqlite(c, extra):
    _, steps, names, shopping = extra
    c.execute("BEGIN")
    c.executemany(
        "INSERT INTO step_outputs VALUES (?,?,?,?,?)",
        (
            (sid, job, step, step_payload(i), ts.isoformat())
            for sid, job, step, i, ts in steps
        ),
    )
    c.executemany(
        "INSERT INTO ingredient_name_resolutions VALUES (?,?,?,?)",
        ((n, st, '{"grams": 1}', ts.isoformat()) for n, st, ts in names),
    )
    c.executemany(
        "INSERT INTO shopping_list_items VALUES (?,?,?,?,?,?)",
        ((i, u, it, int(ch), x, ts.isoformat()) for i, u, it, ch, x, ts in shopping),
    )
    c.execute("COMMIT")
    c.execute("ANALYZE")


class Backend:
    """The mixed workload's statements, written once with {p} placeholders."""

    def __init__(self, name, connect, placeholder, begin, save, list_fn, search_fn):
        self.name, self.connect, self.p = name, connect, placeholder
        self.begin, self.save = begin, save
        self.list_fn, self.search_fn = list_fn, search_fn

    def sql(self, q):
        return q.replace("{p}", self.p)

    def txn(self, c, statements):
        c.execute(self.begin)
        try:
            for q, params in statements:
                c.execute(self.sql(q), params)
            c.execute("COMMIT")
        except Exception:
            c.execute("ROLLBACK")
            raise


def mixed(be, extra, rids, lock=None, seconds=8.0):
    """Readers plus the scrape runner, ingredient-name worker and shopping edits."""
    jobs, _, _, shopping = extra
    stop = time.perf_counter() + seconds
    lat = {k: [] for k in ("read", "step_output", "save_recipe", "names", "shop")}
    errors = []

    def timed(kind, fn):
        t = time.perf_counter()
        try:
            if lock is None or kind == "read":
                fn()
            else:
                with lock:
                    fn()
        except Exception as e:  # noqa: BLE001
            errors.append(repr(e))
        lat[kind].append((time.perf_counter() - t) * 1000)

    def reader():
        c = be.connect()
        while time.perf_counter() < stop:
            r = random.random()
            if r < 0.3:
                timed("read", lambda: be.list_fn(c))
            elif r < 0.5:
                timed("read", lambda: be.search_fn(c, ["garlic"]))
            elif r < 0.8:
                q = "SELECT * FROM shopping_list_items WHERE user_id = {p}"
                timed("read", lambda: c.execute(be.sql(q), (USER,)).fetchall())
            else:
                q = "SELECT * FROM step_outputs WHERE scrape_job_id = {p}"
                job = random.choice(jobs)
                timed("read", lambda: c.execute(be.sql(q), (job,)).fetchall())

    def scraper():
        c = be.connect()
        i = 0
        while time.perf_counter() < stop:
            job = str(uuid.uuid4())
            for step in range(STEPS_PER_JOB):
                ins = (
                    "INSERT INTO step_outputs VALUES ({p}, {p}, {p}, {p}, {p})",
                    (
                        str(uuid.uuid4()),
                        job,
                        f"step{step}",
                        step_payload(step + i),
                        datetime.now(timezone.utc).isoformat(),
                    ),
                )
                timed("step_output", lambda: be.txn(c, [ins]))
            i += 1
            timed("save_recipe", lambda: be.save(c, random.choice(rids)))

    def names_worker():
        c = be.connect()
        while time.perf_counter() < stop:
            batch = random.sample(range(N_NAMES), 50)
            stmts = [
                (
                    "UPDATE ingredient_name_resolutions SET status = {p}, "
                    "result = {p}, updated_at = {p} WHERE name = {p}",
                    (
                        "resolved",
                        '{"grams": 2}',
                        datetime.now(timezone.utc).isoformat(),
                        f"ingredient name {n}",
                    ),
                )
                for n in batch
            ]
            timed("names", lambda: be.txn(c, stmts))
            time.sleep(0.05)

    def shopper():
        c = be.connect()
        while time.perf_counter() < stop:
            item = random.choice(shopping)[0]
            stmt = (
                "UPDATE shopping_list_items SET checked = NOT checked, "
                "change_xid = change_xid + 1, updated_at = {p} WHERE id = {p}",
                (datetime.now(timezone.utc).isoformat(), item),
            )
            timed("shop", lambda: be.txn(c, [stmt]))
            time.sleep(0.02)

    ths = [threading.Thread(target=reader) for _ in range(4)] + [
        threading.Thread(target=f) for f in (scraper, names_worker, shopper)
    ]
    for t in ths:
        t.start()
    for t in ths:
        t.join()

    def q(xs, p):
        xs = sorted(xs)
        return xs[min(len(xs) - 1, int(len(xs) * p))] if xs else float("nan")

    parts = [
        f"{k} n={len(v)} p50 {q(v, 0.5):.1f} p99 {q(v, 0.99):.1f}"
        for k, v in lat.items()
    ]
    return (
        " | ".join(parts)
        + f" ms | errors {len(errors)}"
        + (f" {errors[0]}" if errors else "")
    )


def blocked_by_bulk(connect, bulk, save, rid):
    """Duration of a bulk rewrite, and how long a small save waits behind it."""
    done = {}

    def run_bulk():
        c = connect()
        t = time.perf_counter()
        bulk(c)
        done["bulk"] = (time.perf_counter() - t) * 1000

    th = threading.Thread(target=run_bulk)
    th.start()
    time.sleep(0.02)
    c = connect()
    t = time.perf_counter()
    save(c, rid)
    done["save"] = (time.perf_counter() - t) * 1000
    th.join()
    return (
        f"bulk rewrite {done['bulk']:.0f} ms, "
        f"concurrent single save took {done['save']:.1f} ms"
    )


def main():
    snaps = load_snapshots()
    data = gen_rows(snaps)
    rids = [r["id"] for r in data[1]]
    t = time.perf_counter()
    setup_pg(data)
    print(f"[setup] pg load {time.perf_counter() - t:.1f}s")
    t = time.perf_counter()
    sl = setup_sqlite(data)
    print(
        f"[setup] sqlite load {time.perf_counter() - t:.1f}s, file "
        f"{os.path.getsize(SQLITE_PATH) / 1e6:.1f} MB"
    )
    pg = psycopg.connect(PG_DSN, autocommit=True)
    print(f"== N={N} current recipes, {N * VERSIONS} versions ==")
    print("list page+count   pg     ", fmt(timeit(lambda: pg_list(pg), 50)))
    print("list page+count   sqlite ", fmt(timeit(lambda: sl_list(sl), 50)))
    for toks in (["chicken"], ["garlic", "butter"], ["brulee"], ["xqzv"]):
        # Compare which recipes matched, not just how many.
        ids_pg = {row[0] for row in pg_search(pg, toks)}
        ids_sc = {row[0] for row in sl_search_scan(sl, toks)}
        ids_ft = {row[0] for row in sl_search_fts(sl, toks)}
        same = ids_pg == ids_sc == ids_ft
        print(
            f"search {' '.join(toks):14s} rows pg={len(ids_pg)} scan={len(ids_sc)} "
            f"fts={len(ids_ft)} same_ids={same}"
        )
        assert same, f"search results differ for {toks}"
        print("   pg trgm+ILIKE     ", fmt(timeit(lambda: pg_search(pg, toks), 20)))
        print("   sqlite scan+UDF   ", fmt(timeit(lambda: sl_search_scan(sl, toks), 5)))
        print("   sqlite fts5 trgm  ", fmt(timeit(lambda: sl_search_fts(sl, toks), 20)))
    print("full sync sweep   pg     ", fmt(timeit(lambda: pg_sync_sweep(pg), 5)))
    print("full sync sweep   sqlite ", fmt(timeit(lambda: sl_sync_sweep(sl), 5)))
    print(
        "single save       pg     ",
        fmt(timeit(lambda: pg_save(pg, random.choice(rids)), 50)),
    )
    print(
        "single save       sqlite ",
        fmt(timeit(lambda: sl_save(sl, random.choice(rids)), 50)),
    )
    pg_conn = lambda: psycopg.connect(PG_DSN, autocommit=True)  # noqa: E731
    print(
        "concurrent pg     ",
        concurrent(
            pg_conn, [pg_list, lambda c: pg_search(c, ["garlic"])], pg_save, rids
        ),
    )
    print(
        "concurrent sqlite ",
        concurrent(
            sqlite_connect,
            [sl_list, lambda c: sl_search_fts(c, ["garlic"])],
            sl_save,
            rids,
        ),
    )
    print(
        "concurrent sqlite (one writer mutex) ",
        concurrent(
            sqlite_connect,
            [sl_list, lambda c: sl_search_fts(c, ["garlic"])],
            sl_save_serialized,
            rids,
        ),
    )
    extra = extra_rows(N)
    t = time.perf_counter()
    seed_extra_pg(extra)
    seed_extra_sqlite(sl, extra)
    print(f"[setup] non-recipe tables {time.perf_counter() - t:.1f}s")
    size = pg.execute("SELECT pg_database_size(current_database())").fetchone()[0]
    print(
        f"[size] pg database {size / 1e6:.0f} MB, sqlite file "
        f"{os.path.getsize(SQLITE_PATH) / 1e6:.0f} MB"
    )
    pg_be = Backend("pg", pg_conn, "%s", "BEGIN", pg_save, pg_list, pg_search)
    sl_be = Backend(
        "sqlite",
        sqlite_connect,
        "?",
        "BEGIN IMMEDIATE",
        sl_save,
        sl_list,
        sl_search_fts,
    )
    print("mixed pg                ", mixed(pg_be, extra, rids))
    print("mixed sqlite (mutex)    ", mixed(sl_be, extra, rids, lock=WRITE_LOCK))
    print("bulk pg     ", blocked_by_bulk(pg_conn, pg_bulk_rewrite, pg_save, rids[0]))
    print(
        "bulk sqlite ",
        blocked_by_bulk(sqlite_connect, sl_bulk_rewrite, sl_save, rids[0]),
    )


main()
