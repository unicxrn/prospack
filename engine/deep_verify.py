"""Deep-verify an exFAT-wrapped .ffpfsc against its source folder.

Decompresses the image through MkPFS's own reader, walks the inner exFAT and
compares every file byte-for-byte with the source tree. Reads only; never
writes to the image or the source.

Output: one JSON object per line on stdout.
  {"type":"plan","files":N,"bytes":B}
  {"type":"progress","files":n,"bytes":b}
  {"type":"problem","kind":"missing|extra|size|content|read","path":"...","detail":"..."}
  {"type":"done","ok":true|false,"files":N,"bytes":B,"problems":P}
"""

from __future__ import annotations

import json
import multiprocessing as mp
import os
import sys
import time
from pathlib import Path

CHUNK = 4 * 1024 * 1024


def emit(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def open_reader(image: Path):
    from mkpfs.exfat import ExfatReader
    from mkpfs.pfs import open_inner_file_view

    opened = open_inner_file_view(image)
    if opened is None:
        raise RuntimeError("image is not a single-file PFSC container (expected an exFAT-wrapped .ffpfsc)")
    view, fh, _name = opened
    return ExfatReader(view), fh


def scan_source(root: Path) -> dict[str, int]:
    out: dict[str, int] = {}
    for dirpath, _dirs, files in os.walk(root):
        for name in files:
            full = Path(dirpath) / name
            if full.is_symlink():
                continue
            out[full.relative_to(root).as_posix()] = full.stat().st_size
    return out


def worker(image: str, source: str, paths: list[str], q) -> None:
    try:
        reader, fh = open_reader(Path(image))
    except Exception as exc:  # noqa: BLE001
        q.put(("fatal", str(exc)))
        return
    try:
        entries = {e.rel_path.lstrip("/"): e for e in reader.iter_files()}
        for rel in paths:
            entry = entries[rel]
            done = 0
            try:
                with open(Path(source) / rel, "rb") as src:
                    for chunk in reader.read_file(entry, CHUNK):
                        ref = src.read(len(chunk))
                        if ref != chunk:
                            off = done + next((i for i, (a, b) in enumerate(zip(ref, chunk)) if a != b), min(len(ref), len(chunk)))
                            q.put(("problem", "content", rel, f"first difference at byte {off}"))
                            break
                        done += len(chunk)
                        q.put(("bytes", len(chunk)))
            except Exception as exc:  # noqa: BLE001
                q.put(("problem", "read", rel, str(exc)))
            q.put(("file", entry.length - done))
    finally:
        fh.close()
        q.put(("exit",))


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: deep_verify.py IMAGE SOURCE_DIR [WORKERS]", file=sys.stderr)
        return 2
    image, source = Path(sys.argv[1]), Path(sys.argv[2])
    workers = int(sys.argv[3]) if len(sys.argv) > 3 else max(1, min(8, (os.cpu_count() or 2) // 2))

    try:
        reader, fh = open_reader(image)
        inner = {e.rel_path.lstrip("/"): e.length for e in reader.iter_files()}
        fh.close()
    except Exception as exc:  # noqa: BLE001
        emit({"type": "problem", "kind": "read", "path": str(image), "detail": str(exc)})
        emit({"type": "done", "ok": False, "files": 0, "bytes": 0, "problems": 1})
        return 1

    src = scan_source(source)
    problems = 0
    for rel in sorted(src.keys() - inner.keys()):
        emit({"type": "problem", "kind": "missing", "path": rel, "detail": "in dump but not in image"}); problems += 1
    for rel in sorted(inner.keys() - src.keys()):
        emit({"type": "problem", "kind": "extra", "path": rel, "detail": "in image but not in dump"}); problems += 1
    common = sorted(src.keys() & inner.keys())
    for rel in common:
        if src[rel] != inner[rel]:
            emit({"type": "problem", "kind": "size", "path": rel, "detail": f"dump {src[rel]} B, image {inner[rel]} B"}); problems += 1
    to_check = [r for r in common if src[r] == inner[r]]
    total_bytes = sum(inner[r] for r in to_check)
    emit({"type": "plan", "files": len(to_check), "bytes": total_bytes, "workers": workers})

    # Largest-first round robin keeps workers evenly loaded.
    buckets: list[list[str]] = [[] for _ in range(workers)]
    loads = [0] * workers
    for rel in sorted(to_check, key=lambda r: inner[r], reverse=True):
        i = loads.index(min(loads)); buckets[i].append(rel); loads[i] += inner[rel]

    ctx = mp.get_context("spawn")
    q = ctx.Queue(maxsize=4096)
    procs = [ctx.Process(target=worker, args=(str(image), str(source), b, q)) for b in buckets if b]
    for p in procs:
        p.start()

    files_done = bytes_done = 0
    alive = len(procs)
    last = 0.0
    while alive:
        msg = q.get()
        kind = msg[0]
        if kind == "bytes":
            bytes_done += msg[1]
        elif kind == "file":
            files_done += 1
            bytes_done += msg[1]  # count the unread tail of a failed file so totals line up
        elif kind == "problem":
            problems += 1
            emit({"type": "problem", "kind": msg[1], "path": msg[2], "detail": msg[3]})
        elif kind == "fatal":
            problems += 1
            emit({"type": "problem", "kind": "read", "path": str(image), "detail": msg[1]})
        elif kind == "exit":
            alive -= 1
        now = time.monotonic()
        if now - last > 0.25:
            emit({"type": "progress", "files": files_done, "bytes": bytes_done})
            last = now
    for p in procs:
        p.join()
    emit({"type": "progress", "files": files_done, "bytes": bytes_done})
    emit({"type": "done", "ok": problems == 0, "files": files_done, "bytes": bytes_done, "problems": problems})
    return 0 if problems == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
