# Contributing

## Layout

| Path | What |
| --- | --- |
| `src/` | The app UI (Svelte 5). `src/lib/mock.js` simulates the backend so `npm run dev` works in a browser. |
| `src-tauri/src/` | The Rust backend: `pipeline.rs` (the job queue), `engine.rs`, `ftp.rs`, `ps5.rs`, `discover.rs`, `dump.rs`, `settings.rs`. |
| `engine/` | The Python engine: MkPFS plus the deep verifier, frozen into `prospack-engine` by `build.py`. |
| `installer/` | "Prospack Setup", the Windows installer app. It embeds the NSIS installer and runs it silently. |
| `tests/fake_ps5.py` | A fake PS5 FTP server (with ftpsrv's quirks) and a fake ShadowMountPlus, for end-to-end tests. |

## Development

```bash
npm install
python3 -m venv .venv && .venv/bin/pip install -r engine/requirements-build.txt pyftpdlib
.venv/bin/python engine/build.py     # needed once: Tauri expects the sidecar to exist
npm run tauri dev
```

To work on the UI only, run `npm run dev` and open http://localhost:1430. Add `?setup` to
the URL to see the first-run wizard.

Engine lookup order:
1. `prospack-engine` next to the app executable (release builds).
2. `$PROSPACK_ENGINE`.
3. A developer venv in the app data folder, which runs `engine/prospack_engine.py`.

## Tests

```bash
cd src-tauri
cargo test                                   # unit tests
```

The end-to-end tests run the whole pipeline (compress, verify, upload, mount) against the
fake PS5. They never touch a real console. They need a scratch folder containing:

- `mkvenv/`: a venv with `engine/requirements.txt` and `pyftpdlib`;
- `fake_ps5.py`: copied from `tests/`;
- `fakedump/PPSA00001-app`: from `python engine/make_fake_dump.py <scratch>/fakedump/PPSA00001-app`.

```bash
PACKER_E2E_SCRATCH=<scratch> PROSPACK_ENGINE=$PWD/binaries/prospack-engine-<triple> cargo test -- --include-ignored
```

The scenarios covered are:
- a full run;
- an identical re-upload being skipped;
- a tampered image blocked at Verify;
- "Compress again";
- resuming an interrupted upload;
- Cancel killing the engine;
- the wizard's checks.

`discover_lan` (also ignored) prints the FTP servers on your real network. It's read-only.

## Things the PS5 does differently

The PS5's usual FTP server, ftpsrv 1.6.x, differs from standard FTP in ways the code relies on:

- RNTO, DELE and MKD reply **226** instead of 250/257.
- `SIZE` on a missing file returns `213 18446744073709551615` (-1), so existence is checked with `LIST`.
- STOR doesn't truncate before writing, so temporary files are deleted before they're rewritten.

The ShadowMountPlus details:
- It ignores dot-files, which is why uploads use `.<TITLE>.ffpfsc.prospack-part`.
- It only reliably notices a new image when `manual.lst` changes.

## Releases

Push a tag such as `v0.1.0`. `.github/workflows/release.yml` builds Linux and Windows,
runs the tests (including the end-to-end tests with the frozen engine), builds Prospack
Setup, smoke-tests an unattended install and uninstall on Windows, and creates a **draft**
release for you to check and publish.
