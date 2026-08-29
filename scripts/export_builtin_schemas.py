#!/usr/bin/env python3
"""Export the built-in schemas from the live app database into the
P4 portable format (biblio-schema v1), byte-compatible with the
`schema_export` command's envelope."""

import json
import sqlite3
import sys
from pathlib import Path

DB = Path.home() / "Library/Application Support/io.augite.biblio/biblio.db"
OUT = Path(__file__).resolve().parent.parent / "schema-exports"

# Open read-only so a running app is never disturbed (WAL-safe).
conn = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
conn.row_factory = sqlite3.Row

rows = conn.execute(
    "SELECT id, name, icon, description, accepted_extensions, pipeline_template "
    "FROM schemas WHERE is_builtin = 1 ORDER BY sort_order, id"
).fetchall()

OUT.mkdir(exist_ok=True)
written = []
for row in rows:
    sid = row["id"]
    fields = [
        {
            "field_key": f["field_key"],
            "semantic": f["semantic"],
            "field_type": f["field_type"],
            "label": f["label"],
            "options": f["options"],
            "form_visible": bool(f["form_visible"]),
            "card_visible": bool(f["card_visible"]),
            "sortable": bool(f["sortable"]),
            "filterable": bool(f["filterable"]),
            "required": bool(f["required"]),
            "order_index": f["order_index"],
        }
        for f in conn.execute(
            "SELECT field_key, semantic, field_type, label, options, form_visible, "
            "card_visible, sortable, filterable, required, order_index "
            "FROM schema_fields WHERE schema_id = ? ORDER BY order_index, id",
            (sid,),
        )
    ]
    steps = [
        {
            "step_key": s["step_key"],
            "label": s["label"],
            "enabled": bool(s["enabled"]),
            "order_index": s["order_index"],
        }
        for s in conn.execute(
            "SELECT step_key, label, enabled, order_index "
            "FROM schema_pipeline_steps WHERE schema_id = ? ORDER BY order_index, id",
            (sid,),
        )
    ]
    doc = {
        "format": "biblio-schema",
        "version": 1,
        "schema": {
            "id": sid,
            "name": row["name"],
            "icon": row["icon"],
            "description": row["description"],
            "accepted_extensions": json.loads(row["accepted_extensions"]),
            "pipeline_template": row["pipeline_template"],
            "fields": fields,
            "pipeline_steps": steps,
        },
    }
    path = OUT / f"{sid}.schema.json"
    path.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    written.append(path)
    print(f"{sid}: {len(fields)} fields, {len(steps)} steps -> {path.name}")

if not rows:
    sys.exit("No built-in schemas found — was migration v6 applied?")
