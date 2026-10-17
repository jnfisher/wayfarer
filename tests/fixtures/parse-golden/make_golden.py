#!/usr/bin/env python3
"""Produce the golden expected events for wayfarer-parse from the prototype's parser.

Run once, by hand, in a checkout of jnfisher/mmud-exploration (never in CI):

    python3 -I make_golden.py <mmud-exploration>/test/play <wayfarer>/tests/fixtures

It imports `patterns.py` from the first directory and reads only wayfarer's own fixture copies
from the second (world export, captures, transcripts), so both sides see the same input. The
outputs land next to this script:

- `registry.jsonl.gz`: every pattern of the registry in its list order (static and generated):
  name, cat, prio, spec, key (None for the "always" bucket), the regex source, and for
  generated patterns the WCCMSG text, its number and the fields its regex gives on its own text.
- `events/<fixture>.jsonl.gz`: every event `Registry.iter_events` yields, in order, with the
  index of the line it came from (`n`), name, cat, fields and line.
- `patterns.json`: `patterns.py export` output (the static table, tables, joins, prompt forms).
- `static-order.txt`: the static patterns' names in `patterns.STATIC` order.
- `summary.json`: counts, the unclassified rate and the Python timing per fixture.

`patterns.load_world` reads the module's raw Btrieve rows through `mmud_data._rows`, which this
checkout does not have; the script rebuilds those rows from the export, writing each field the
loader reads at its struct offset (the same offsets as `mmud_data.RECORDS`).
"""
import gzip
import json
import os
import re
import struct
import sys
import time

PLAY, FIX = (os.path.abspath(p) for p in sys.argv[1:3])
OUT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, PLAY)
import mmud_data as md  # noqa: E402
import patterns as pt  # noqa: E402

WORLD = os.path.join(FIX, "world")


def _str_at(buf, off, n, s):
    b = s.encode("cp437")[: n - 1]
    buf[off:off + len(b)] = b


def _rows(name):
    rows = []
    for x in md.load_export(WORLD, name):
        if name == "WCCKNMSR":
            d = bytearray(732)
            _str_at(d, 0x36, 30, x["name"])
            struct.pack_into("<ii", d, 0xB2, x["arrivemsg"], x["deathmsg"])
            d[0x11E:0x123] = bytes(x["atktypes"])
            struct.pack_into("<5i", d, 0x146, *x["hitmsgs"])
            struct.pack_into("<5i", d, 0x15A, *x["defmsgs"])
            struct.pack_into("<5i", d, 0x16E, *x["missmsgs"])
        elif name == "WCCITEMS":
            d = bytearray(1061)
            struct.pack_into("<ii", d, 0x404, x["hitmsg"], x["missmsg"])
            struct.pack_into("<i", d, 0x410, x["destroymsg"])
        elif name == "WCCSPELS":
            d = bytearray(253)
            struct.pack_into("<i", d, 0xE7, x["msgnum"])
        else:
            raise KeyError(name)
        rows.append(bytes(d))
    return rows


md._rows = _rows


def jdump(obj):
    return json.dumps(obj, ensure_ascii=False, sort_keys=True)


def write_gz(path, lines):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    # mtime=0: the same input gives the same bytes
    with open(path, "wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as f:
        for line in lines:
            f.write((line + "\n").encode("utf-8"))


def main():
    # A fresh `patterns.py export` (the copy in ../patterns predates 37 static patterns), and the
    # static table's source order, which the export sorts away and ties between patterns need.
    text = json.dumps(pt.export_registry(False), indent=1, ensure_ascii=False, sort_keys=False) + "\n"
    with open(os.path.join(OUT, "patterns.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    with open(os.path.join(OUT, "static-order.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(name + "\n" for _cat, name, *_rest in pt.STATIC))

    t0 = time.time()
    world = md.World(world_json=WORLD)
    reg = pt.Registry.load(world=world)
    build_s = time.time() - t0

    reg_lines = []
    for order, p in enumerate(reg.patterns):
        rec = {"order": order, "name": p.name, "cat": p.cat, "prio": p.prio, "spec": p.spec,
               "key": p.key, "rx": p.rx.pattern, "generated": p.extra is not None}
        if p.extra is not None:
            m = p.rx.fullmatch(p.extra)
            rec.update(text=p.extra, msgnum=p.msgnum,
                       self_match=None if m is None else {k: v for k, v in m.groupdict().items() if v is not None})
        reg_lines.append(jdump(rec))
    write_gz(os.path.join(OUT, "registry.jsonl.gz"), reg_lines)

    summary = {"registry": {"patterns": len(reg.patterns),
                            "generated": sum(1 for p in reg.patterns if p.extra is not None),
                            "keys": len(reg.index), "always": len(reg.always),
                            "room_titles": len(reg.room_names), "build_seconds": round(build_s, 3)},
               "fixtures": {}}

    fixtures = []
    for d in sorted(os.listdir(os.path.join(FIX, "captures"))):
        f = os.path.join(FIX, "captures", d, "raw.bin")
        if os.path.exists(f):
            with open(f, "rb") as fh:
                fixtures.append((f"capture-{d}", pt.raw_to_lines(fh.read())))
    for name in sorted(os.listdir(os.path.join(FIX, "transcripts"))):
        if not name.endswith(".txt"):
            continue
        with open(os.path.join(FIX, "transcripts", name), encoding="utf-8", errors="replace") as fh:
            src = fh.read().split("\n")
        lines = []
        for line in src:                      # as patterns.selftest reads a transcript
            if line.startswith("### "):
                continue
            if line.startswith(">>> "):
                m = re.match(r">>> '(.*)'$", line)
                if m:
                    lines.append(pt.sent_marker(m.group(1).encode().decode("unicode_escape")))
                continue
            lines.append(line)
        fixtures.append((f"transcript-{name[:-4]}", lines))

    total_lines = total_events = total_unknown = 0
    total_s = 0.0
    for fname, lines in fixtures:
        t0 = time.perf_counter()
        evs = list(reg.iter_events(lines))
        dt = time.perf_counter() - t0
        out = [jdump({"n": e.lineno, "name": e.name, "cat": e.cat, "fields": e.fields, "line": e.line})
               for e in evs]
        write_gz(os.path.join(OUT, "events", fname + ".jsonl.gz"), out)
        unknown = sum(1 for e in evs if e.name == "unknown")
        summary["fixtures"][fname] = {"lines": len(lines), "events": len(evs), "unknown": unknown,
                                      "us_per_line": round(dt / max(1, len(lines)) * 1e6, 2)}
        total_lines += len(lines)
        total_events += len(evs)
        total_unknown += unknown
        total_s += dt
    summary["total"] = {"lines": total_lines, "events": total_events, "unknown": total_unknown,
                        "unknown_rate": round(total_unknown / max(1, total_events), 5),
                        "us_per_line": round(total_s / max(1, total_lines) * 1e6, 2)}
    with open(os.path.join(OUT, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=1, sort_keys=True)
        f.write("\n")
    print(json.dumps(summary["total"]), json.dumps(summary["registry"]))


if __name__ == "__main__":
    main()
