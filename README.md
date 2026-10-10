# Packer

Compresses a PS5 game dump into a single `.ffpfsc` image on your PC, checks it file by
file against the dump, uploads it to the PS5 over FTP and gets ShadowMountPlus to mount it.
It does on the PC what Game Compressor does on the console, but faster, and it never
deletes the dump.

## How a job runs

1. **Compress.** [MkPFS](https://github.com/PSBrew/MkPFS) 1.1.0 runs `pack folder`,
   producing an exFAT image inside a PFSC container (the layout Game Compressor calls
   "exFAT"). The image is written to `.work-<TITLE>/` in the image folder and moved into
   place only when MkPFS finishes. For APR Emu games, MkPFS checks `ampr_emu.index` and
   creates it **inside the dump folder** if it's missing or invalid. That is the only
   write to the dump.
2. **Verify.** `resources/deep_verify.py` decompresses the image through MkPFS's own
   reader, walks the inner exFAT and compares every file byte for byte with the dump,
   in parallel. Any missing, extra, resized or different file fails the job, and nothing
   gets uploaded. "Compress again" deletes the bad image and rebuilds it.
3. **Upload.** One FTP transfer to `<upload folder>/.<TITLE>.ffpfsc.packer-part`.
   ShadowMountPlus ignores dot-files, so a partial file is never mounted. The upload
   resumes with `REST` after a drop, is size-checked, then renamed to `<TITLE>.ffpfsc`.
   An existing image of a different size is never overwritten.
4. **Mount.** The upload writes the same hints Game Compressor writes:
   `image_sector=<TITLE>.ffpfsc:65536` in `/data/shadowmount/autotune.ini` and
   `image_ro=<TITLE>.exfat` in `config.ini`. Both files are backed up first to
   `~/.local/share/ps5-packer/backups/` and replaced atomically. Packer then polls
   `/user/app/<TITLE>/mount_img.lnk` for up to two minutes.

## Using it

- Drop a dump folder on the window, press Ctrl+O, or start it with folders as
  arguments: `ps5-packer /path/to/PPSA01234-app`.
- Settings (Ctrl+,) has the PS5 address and ports, the image folder, compression level,
  and whether to upload automatically after Verify.
- On first launch, "Install MkPFS" creates a private Python venv in
  `~/.local/share/ps5-packer/engine`. Python 3 must be installed.

## Development

```bash
npm install
npm run tauri dev        # desktop app
npm run dev              # browser preview with a simulated backend (src/lib/mock.js)
npx tauri build --bundles deb
```

Tests (`src-tauri`):

```bash
cargo test                                   # unit tests
PACKER_E2E_SCRATCH=<dir> cargo test -- --ignored
```

The ignored tests need `<dir>` to contain:
- `mkvenv/`: a venv with `mkpfs==1.1.0` and `pyftpdlib`
- `fake_ps5.py` (copy from `tests/`)
- `fakedump/PPSA00001-app`

They run the whole pipeline against a local fake PS5: an FTP server answering like
`ftpsrv.elf` (RNTO/DELE reply 226) and a stand-in ShadowMountPlus. The scenarios are a
full run, an identical re-upload being skipped, a tampered image being blocked at Verify,
"Compress again", and resuming an interrupted upload. Another ignored test checks that
Cancel kills MkPFS promptly.

## Notes

- The PS5's `ftpsrv` STOR doesn't truncate before writing, so small temp files are
  deleted before they're rewritten.
- Game Compressor 1.0.4's Validate can't remount images under ShadowMountPlus 1.7. It
  reads only the first line of `mount_img.lnk`, which 1.7 writes outermost-first. This
  doesn't affect Packer, which checks every line.
