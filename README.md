<div align="center">

<img src="src-tauri/icons/128x128@2x.png" width="104" alt="Prospack icon">

# Prospack

Pack, verify and send PS5 game dumps as `.ffpfsc`, from your PC.

[![Release](https://img.shields.io/github/v/release/unicxrn/prospack?style=flat-square&color=9b7bff&label=release)](https://github.com/unicxrn/prospack/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/unicxrn/prospack/total?style=flat-square&color=3a3a40)](https://github.com/unicxrn/prospack/releases)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-3a3a40?style=flat-square)](#download)
[![License](https://img.shields.io/github/license/unicxrn/prospack?style=flat-square&color=3a3a40)](LICENSE)

<img src="docs/screenshot.png" alt="Prospack compressing a game" width="860">

</div>

Prospack takes a PS5 game dump, compresses it into a single `.ffpfsc` image on your PC,
checks every file in that image against the original, uploads it to your PS5 and gets
ShadowMountPlus to mount it. It's the same kind of image Game Compressor makes on the
console, built on your PC's CPU instead, so it's a lot quicker. You also send one file
that's usually 40 to 60% smaller than the dump, instead of tens of thousands of loose ones.

## Download

| Platform | Download |
| --- | --- |
| **Windows 10 and 11** | [`Prospack-Setup.exe`](https://github.com/unicxrn/prospack/releases/latest) |
| **Linux** | [`.AppImage`](https://github.com/unicxrn/prospack/releases/latest) for any distro, or the `.deb` for Debian and Ubuntu |

Python and MkPFS are built in, so there's nothing else to install. Prospack checks for new
versions when it starts and can update itself.

The Windows installer isn't code-signed, so SmartScreen may say "Windows protected your
PC". Click **More info**, then **Run anyway**. The checksums for every download are on the
release page if you want to verify it first.

## What you need

- A jailbroken PS5 with an FTP server running: ftpsrv (port 2121) or etaHEN (port 1337).
- ShadowMountPlus running on the PS5.
- A game dump: the folder with `sce_sys/` and `eboot.bin` in it, like `PPSA01234-app`.
- Enough free space on your PC for the image while it's being written. It can need up to
  the size of the dump.

## Using it

The first time you open Prospack, setup scans your network for the PS5, checks that
ShadowMountPlus is there, and asks where to keep images. After that, drag a dump folder
onto the window (or press <kbd>Ctrl</kbd>+<kbd>O</kbd>) and leave it running. Each game
goes through four steps:

| Step | What happens |
| --- | --- |
| **Compress** | [MkPFS](https://github.com/PSBrew/MkPFS) packs the dump into `<TITLE>.ffpfsc`: an exFAT image inside a compressed PFS container. |
| **Verify** | Every file is read back out of the image and compared with the dump, byte for byte. If anything differs, the job stops before it uploads. |
| **Upload** | The image goes to `/data/homebrew` under a hidden name and is renamed once it's complete. Interrupted uploads resume, and Prospack won't overwrite an image that's already there. |
| **Mount** | Prospack registers the image with ShadowMountPlus and waits for the game to appear. |

You can queue as many games as you like. The <kbd>?</kbd> button in the app explains every
screen and walks through the common problems.

<details>
<summary><b>What Prospack changes on your PS5</b></summary>
<br>

It writes the same settings Game Compressor writes for its own images, and backs up every
file on your PC before changing it:

| File | Change |
| --- | --- |
| `/data/homebrew/<TITLE>.ffpfsc` | The image |
| `/data/shadowmount/autotune.ini` | `image_sector=<TITLE>.ffpfsc:65536` |
| `/data/shadowmount/config.ini` | `image_ro=<TITLE>.exfat` |
| `/data/shadowmount/manual.lst` | The image path. Rewriting this file is what makes ShadowMountPlus scan for it. |
| `/data/GameCompressor/validations/` | A record that makes Game Compressor show the game as validated, with the space saved. Only if Game Compressor is installed. |

Your dump folder is only read. The one exception is an APR Emu game that has no
`ampr_emu.index`: MkPFS adds one, because the game needs it.

</details>

<details>
<summary><b>Troubleshooting</b></summary>
<br>

#### Prospack can't find my PS5

Check the PS5 is on and running its FTP server, and that
the PC is on the same network. Guest Wi-Fi and VPNs often keep them apart. You can type the
IP in yourself under Settings → Run setup again (the PS5 shows it under Settings → Network).

#### Verify failed

The image didn't match the dump, so nothing was uploaded. Use
**Compress again**. If it keeps failing, copy the dump again; it may be incomplete or on a
failing drive.

#### ShadowMountPlus couldn't register the game

The image is on the PS5, but ShadowMountPlus couldn't add it to the home screen. If the
message mentions a lost system hook, that's a ShadowMountPlus 1.7beta4 (and older) bug
after rest mode: update to 1.7beta5 or newer, or restart ShadowMountPlus, then use
**Check again**.

#### Mount not confirmed

The image made it to the PS5, but ShadowMountPlus hadn't mounted
it after five minutes. It often turns up a little later. Use **Check again**, or restart
ShadowMountPlus.

#### "The PS5 already has a different image"

Delete or rename the old `<TITLE>.ffpfsc` on
the PS5, then retry.

#### The game shows up but won't start

That's usually a firmware, backport or
ShadowMountPlus version issue rather than the image. ShadowMountPlus logs to
`/data/shadowmount/debug.log`.

</details>

## Contributing

Bug reports and pull requests are welcome. When you report a bug, the activity log for the
game and your ShadowMountPlus version help a lot.

To work on Prospack you need Node 22, Rust and Python 3.12:

```bash
npm install
pip install -r engine/requirements-build.txt && python engine/build.py
npm run tauri dev
```

`npm run dev` runs the interface on its own in a browser, with a simulated PS5, which is
the quickest way to work on the UI. [CONTRIBUTING.md](CONTRIBUTING.md) covers the project
layout, the tests (which run against a fake PS5, never a real one) and the quirks of the
PS5's FTP server.

## Credits

Prospack is built on [MkPFS](https://github.com/PSBrew/MkPFS) and made to work alongside
[ShadowMountPlus](https://github.com/drakmor/ShadowMountPlus),
[Game Compressor](https://github.com/juma-sayeh/PS5-Game-Compressor) and
[ftpsrv](https://github.com/ps5-payload-dev/ftpsrv). Thanks to everyone who works on them.

Please only use dumps of games you own.

## License

[GPL-3.0](LICENSE)
