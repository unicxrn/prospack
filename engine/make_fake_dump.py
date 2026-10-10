"""Write a small, reproducible fake PS5 dump for tests and CI (no real game data).

    python engine/make_fake_dump.py OUT_DIR/PPSA00001-app
"""

import json
import os
import random
import struct
import sys
import zlib
from pathlib import Path


def png(path: Path, w: int, h: int, rgb: tuple[int, int, int]) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    rows = b"".join(b"\0" + bytes(rgb) * w for _ in range(h))
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows, 9))
        + chunk(b"IEND", b"")
    )


def main() -> int:
    root = Path(sys.argv[1])
    title = root.name.split("-")[0] if root.name[:4].isalpha() else "PPSA00001"
    rnd = random.Random(1234)
    (root / "sce_sys").mkdir(parents=True, exist_ok=True)
    (root / "fakelib").mkdir(exist_ok=True)
    (root / "data/prein/bg").mkdir(parents=True, exist_ok=True)
    (root / "sce_sys/param.json").write_text(json.dumps({
        "titleId": title,
        "contentVersion": "01.000.000",
        "localizedParameters": {"defaultLanguage": "en-US", "en-US": {"titleName": "TEST GAME"}},
    }))
    png(root / "sce_sys/icon0.png", 512, 512, (40, 90, 220))
    png(root / "sce_sys/pic0.png", 1280, 720, (30, 30, 60))
    (root / "eboot.bin").write_bytes(rnd.randbytes(2 * 1024 * 1024))
    (root / "fakelib/libSceAmpr.sprx").write_bytes(rnd.randbytes(64 * 1024))
    for i in range(300):
        (root / f"data/prein/bg/small_{i}.txt").write_bytes(b"a" * rnd.randrange(1, 90000))
    with open(root / "data/prein/bg/big_text.bin", "wb") as f:
        line = b"level data compressible\n"
        f.write(line * (40 * 1024 * 1024 // len(line)))
    (root / "data/prein/bg/big_random.bin").write_bytes(rnd.randbytes(8 * 1024 * 1024))
    print(root)
    return 0


if __name__ == "__main__":
    sys.exit(main())
