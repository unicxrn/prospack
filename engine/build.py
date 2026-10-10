"""Freeze the engine into src-tauri/binaries/prospack-engine-<target-triple>[.exe].

Tauri's sidecar convention needs the Rust target triple in the file name.
Run with the Python that has engine/requirements-build.txt installed.
"""

import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ENGINE = ROOT / "engine"
OUT = ROOT / "src-tauri" / "binaries"


def host_triple() -> str:
    out = subprocess.run(["rustc", "-vV"], check=True, capture_output=True, text=True).stdout
    return next(l.split(": ", 1)[1] for l in out.splitlines() if l.startswith("host: "))


def main() -> int:
    work = ROOT / "target-engine"
    subprocess.run(
        [
            sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean", "--onefile",
            "--name", "prospack-engine",
            "--collect-all", "mkpfs",
            "--collect-all", "zlib_ng",
            "--hidden-import", "deep_verify",
            "--paths", str(ENGINE),
            "--distpath", str(work / "dist"),
            "--workpath", str(work / "build"),
            "--specpath", str(work),
            str(ENGINE / "prospack_engine.py"),
        ],
        check=True,
    )
    exe = ".exe" if platform.system() == "Windows" else ""
    OUT.mkdir(parents=True, exist_ok=True)
    dest = OUT / f"prospack-engine-{host_triple()}{exe}"
    shutil.copy2(work / "dist" / f"prospack-engine{exe}", dest)
    if not exe:
        os.chmod(dest, 0o755)
    print(dest)
    return 0


if __name__ == "__main__":
    sys.exit(main())
