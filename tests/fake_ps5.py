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

def fake_smp():
    hb = os.path.join(root, "data/homebrew")
    while True:
        for n in os.listdir(hb):
            if n.endswith(".ffpfsc") and not n.startswith("."):
                tid = n[:9]
                d = os.path.join(root, "user/app", tid); os.makedirs(d, exist_ok=True)
                lnk = os.path.join(d, "mount_img.lnk")
                if not os.path.exists(lnk):
                    time.sleep(4)
                    open(lnk, "w").write(f"/data/homebrew/{n}\n/mnt/shadowmnt/pfsc/{tid}_deadbeef/{tid}.exfat\n")
        time.sleep(1)
threading.Thread(target=fake_smp, daemon=True).start()

class Ps5Handler(FTPHandler):
    """Answers like ftpsrv.elf: RNTO and DELE reply 226 instead of 250."""
    def respond(self, resp, logfun=None):
        cmd = (getattr(self, "_last_cmd", "") or "").upper()
        if cmd in ("RNTO", "DELE") and resp.startswith("250"):
            resp = "226" + resp[3:]
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
