"""Local stand-in for the PS5: anonymous FTP with PS5-like paths + a fake ShadowMountPlus."""
import os, sys, threading, time
from pyftpdlib.authorizers import DummyAuthorizer
from pyftpdlib.handlers import FTPHandler
from pyftpdlib.servers import FTPServer

root, port = sys.argv[1], int(sys.argv[2])
for d in ["data/homebrew", "data/shadowmount", "user/app"]:
    os.makedirs(os.path.join(root, d), exist_ok=True)
cfg = os.path.join(root, "data/shadowmount/config.ini")
if not os.path.exists(cfg):
    open(cfg, "w").write("# ShadowMountPlus\nmount_read_only=1\nimage_ro=OTHER.exfat\n")
    open(os.path.join(root, "data/shadowmount/autotune.ini"), "w").write("image_sector=PPSA00001.ffpfsc:4096\n")
    open(os.path.join(root, "data/shadowmount/debug.log"), "w").write("[00:00:00] ShadowMount+ (fake)\n")

def fake_smp():
    """Mimics ShadowMountPlus 1.7: link files first, then registration, then manual.status.

    Create data/shadowmount/.broken-hook to reproduce a lost ShellCore hook: the link files
    are written but registration fails, so manual.status never says "installed".
    """
    smp = os.path.join(root, "data/shadowmount")
    hb = os.path.join(root, "data/homebrew")
    done = set()
    while True:
        try:
            listed = open(os.path.join(smp, "manual.lst")).read().split()
        except FileNotFoundError:
            listed = []
        for n in os.listdir(hb):
            path = f"/data/homebrew/{n}"
            if not n.endswith(".ffpfsc") or n.startswith(".") or path not in listed or path in done:
                continue
            done.add(path)
            tid = n[:9]
            time.sleep(4)
            d = os.path.join(root, "user/app", tid); os.makedirs(d, exist_ok=True)
            open(os.path.join(d, "mount_img.lnk"), "w").write(f"{path}\n/mnt/shadowmnt/pfsc/{tid}_deadbeef/{tid}.exfat\n")
            with open(os.path.join(smp, "debug.log"), "a") as log:
                log.write(f"[00:00:00]   [ACTION] Installing: Test Game ({tid})\n")
                if os.path.exists(os.path.join(smp, ".broken-hook")):
                    log.write("[00:00:04]   [REG] internal AppInstallTitleDir bridge unavailable\n")
                    log.write(f"[00:00:04] NOTIFY: Register failed: Test Game ({tid})\n")
                    continue
                log.write("[00:00:04]   [REG] Installed NEW!\n")
            with open(os.path.join(smp, "manual.status"), "a") as st:
                st.write(f"installed\t{tid}\t{path}\tTest Game\n")
        time.sleep(1)
threading.Thread(target=fake_smp, daemon=True).start()

class Ps5Handler(FTPHandler):
    """Answers like ftpsrv.elf: RNTO, DELE and MKD reply 226."""
    def respond(self, resp, logfun=None):
        cmd = (getattr(self, "_last_cmd", "") or "").upper()
        if cmd in ("RNTO", "DELE") and resp.startswith("250"):
            resp = "226" + resp[3:]
        if cmd == "MKD" and resp.startswith("257"):
            resp = "226 Directory created"
        return super().respond(resp) if logfun is None else super().respond(resp, logfun)
    def ftp_SIZE(self, path):
        # Mimic ftpsrv v1.6.22: a missing file reports size -1 instead of 550.
        if not self.fs.isfile(path):
            self.respond("213 18446744073709551615")
            return
        return super().ftp_SIZE(path)
    def pre_process_command(self, line, cmd, arg):
        self._last_cmd = cmd
        return super().pre_process_command(line, cmd, arg)

a = DummyAuthorizer(); a.add_anonymous(root, perm="elradfmwMT")
h = Ps5Handler; h.authorizer = a; h.permit_foreign_addresses = True
FTPServer(("127.0.0.1", port), h).serve_forever()
