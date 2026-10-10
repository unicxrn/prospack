# Contributing to Prospack

Thanks for helping out. Bug reports, fixes and new ideas are all welcome.

## Reporting a bug

Open an issue and include:

- what you did and what happened instead
- your OS, your Prospack version and your ShadowMountPlus version
- the game's activity log from the app, if the problem happened during a job

## Setting up

You need Node 22, Rust and Python 3.12.

```bash
npm install
pip install -r engine/requirements-build.txt
python engine/build.py     # builds the compression engine the app runs
npm run tauri dev
```

If you only want to work on the interface, `npm run dev` opens it in a browser with a
simulated PS5. Add `?setup` to the URL to see the setup wizard.

## Where things are

| Path | What it holds |
| --- | --- |
| `src/` | The interface (Svelte) |
| `src-tauri/src/` | The app backend (Rust): the job queue, FTP, PS5 checks |
| `engine/` | The compression engine (Python, using MkPFS) |
| `installer/` | Prospack Setup, the Windows installer app |
| `tests/fake_ps5.py` | A fake PS5 used by the tests |

## Tests

```bash
cd src-tauri && cargo test
```

The end-to-end tests run a whole job against the fake PS5, never a real console. CI runs
them on Linux and Windows for every pull request, so you don't have to set them up
yourself. If you want to run them locally, see the steps in `.github/workflows/ci.yml`.

## Pull requests

- Keep each pull request to one change, and say what it fixes or adds.
- Make sure `cargo test` and `npm run build` pass.
- Check how your change behaves on the PS5's FTP server, ftpsrv. It doesn't behave like a
  standard one: some commands reply with different codes, `SIZE` returns -1 for missing
  files, and uploads don't truncate existing files. `src-tauri/src/ftp.rs` handles these.
