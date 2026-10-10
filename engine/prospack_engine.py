"""Prospack engine: the compression and verification half of Prospack.

Frozen into a single executable (see build.py) and shipped next to the app, so users
don't need Python.

    prospack-engine pack folder [mkpfs options] SOURCE IMAGE
    prospack-engine verify IMAGE SOURCE [WORKERS]
    prospack-engine --version
"""

import multiprocessing
import sys


def main() -> int:
    # Must run first: MkPFS and the verifier start worker processes, which re-enter this
    # executable when frozen.
    multiprocessing.freeze_support()
    if len(sys.argv) < 2 or sys.argv[1] not in ("pack", "verify", "--version"):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    cmd = sys.argv[1]
    if cmd == "--version":
        import importlib.metadata as md

        print(f"mkpfs {md.version('mkpfs')}")
        return 0
    if cmd == "pack":
        from mkpfs.cli import cli_mkpfs_main

        return cli_mkpfs_main(sys.argv[1:])
    import deep_verify

    sys.argv = [sys.argv[0], *sys.argv[2:]]
    return deep_verify.main()


if __name__ == "__main__":
    sys.exit(main())
