//! A small, blocking FTP client: just what talking to the PS5's FTP server needs.
//! Passive mode, binary transfers, resumable uploads.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Ftp {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    peer: SocketAddr,
}

#[derive(Debug)]
pub struct Reply {
    pub code: u16,
    pub text: String,
}

impl Ftp {
    pub fn connect(host: &str, port: u16, connect_timeout: Duration) -> Result<Self, String> {
        let addr = (host, port)
            .to_socket_addrs()
            .map_err(|e| format!("Can't resolve {host}: {e}"))?
            .next()
            .ok_or_else(|| format!("Can't resolve {host}"))?;
        let stream = TcpStream::connect_timeout(&addr, connect_timeout)
            .map_err(|e| format!("No FTP server at {host}:{port} ({e})"))?;
        stream.set_read_timeout(Some(TIMEOUT)).ok();
        stream.set_write_timeout(Some(TIMEOUT)).ok();
        stream.set_nodelay(true).ok();
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        let mut ftp = Self { reader: BufReader::new(stream), writer, peer: addr };
        let hello = ftp.read_reply()?;
        if hello.code != 220 {
            return Err(format!("Unexpected FTP greeting: {} {}", hello.code, hello.text));
        }
        let r = ftp.cmd("USER anonymous")?;
        if r.code == 331 {
            let r = ftp.cmd("PASS anonymous@")?;
            if r.code != 230 && r.code != 202 {
                return Err(format!("FTP login refused: {} {}", r.code, r.text));
            }
        } else if r.code != 230 {
            return Err(format!("FTP login refused: {} {}", r.code, r.text));
        }
        ftp.expect("TYPE I", &[200])?;
        Ok(ftp)
    }

    fn read_reply(&mut self) -> Result<Reply, String> {
        let mut line = String::new();
        let mut read_line = |line: &mut String| -> Result<(), String> {
            line.clear();
            let n = self.reader.read_line(line).map_err(|e| format!("FTP connection lost: {e}"))?;
            if n == 0 {
                return Err("FTP server closed the connection".into());
            }
            Ok(())
        };
        read_line(&mut line)?;
        let code: u16 = line.get(..3).and_then(|c| c.parse().ok()).ok_or_else(|| format!("Bad FTP reply: {line:?}"))?;
        let mut text = line[3..].trim_start_matches([' ', '-']).trim_end().to_string();
        if line.as_bytes().get(3) == Some(&b'-') {
            let end = format!("{code} ");
            loop {
                read_line(&mut line)?;
                text.push('\n');
                text.push_str(line.trim_end());
                if line.starts_with(&end) {
                    break;
                }
            }
        }
        Ok(Reply { code, text })
    }

    pub fn cmd(&mut self, c: &str) -> Result<Reply, String> {
        self.writer
            .write_all(format!("{c}\r\n").as_bytes())
            .map_err(|e| format!("FTP connection lost: {e}"))?;
        self.read_reply()
    }

    fn expect(&mut self, c: &str, ok: &[u16]) -> Result<Reply, String> {
        let r = self.cmd(c)?;
        if ok.contains(&r.code) {
            Ok(r)
        } else {
            let shown = if c.starts_with("PASS") { "PASS ***" } else { c };
            Err(format!("{shown} failed: {} {}", r.code, r.text))
        }
    }

    pub fn noop(&mut self) -> Result<(), String> {
        self.expect("NOOP", &[200]).map(|_| ())
    }

    pub fn size(&mut self, path: &str) -> Result<Option<u64>, String> {
        let r = self.cmd(&format!("SIZE {path}"))?;
        Ok(if r.code == 213 { r.text.trim().parse().ok() } else { None })
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.expect(&format!("RNFR {from}"), &[350])?;
        // ftpsrv (the usual PS5 server) answers RNTO and DELE with 226 rather than 250.
        self.expect(&format!("RNTO {to}"), &[250, 226]).map(|_| ())
    }

    pub fn delete(&mut self, path: &str) -> Result<(), String> {
        self.expect(&format!("DELE {path}"), &[250, 226]).map(|_| ())
    }

    fn pasv(&mut self) -> Result<TcpStream, String> {
        let r = self.expect("PASV", &[227])?;
        let start = r.text.find('(').ok_or("Bad PASV reply")? + 1;
        let end = r.text[start..].find(')').ok_or("Bad PASV reply")? + start;
        let nums: Vec<u16> = r.text[start..end].split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() != 6 {
            return Err(format!("Bad PASV reply: {}", r.text));
        }
        let port = nums[4] * 256 + nums[5];
        // Some console servers announce 0.0.0.0 or an internal address; the control peer is reliable.
        let addr = SocketAddr::new(self.peer.ip(), port);
        let s = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("FTP data connection failed: {e}"))?;
        s.set_read_timeout(Some(TIMEOUT)).ok();
        s.set_write_timeout(Some(TIMEOUT)).ok();
        Ok(s)
    }

    /// Downloads a (small) file. `Ok(None)` when it doesn't exist.
    pub fn retr(&mut self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let mut data = self.pasv()?;
        let r = self.cmd(&format!("RETR {path}"))?;
        if r.code == 550 {
            return Ok(None);
        }
        if r.code != 150 && r.code != 125 {
            return Err(format!("RETR {path} failed: {} {}", r.code, r.text));
        }
        let mut buf = Vec::new();
        data.read_to_end(&mut buf).map_err(|e| format!("Download of {path} failed: {e}"))?;
        drop(data);
        let done = self.read_reply()?;
        if done.code != 226 && done.code != 250 {
            return Err(format!("Download of {path} failed: {} {}", done.code, done.text));
        }
        Ok(Some(buf))
    }

    /// Names in a directory (via LIST, since NLST support varies). Hidden entries included.
    pub fn list_names(&mut self, path: &str) -> Result<Vec<(String, bool)>, String> {
        let mut data = self.pasv()?;
        let r = self.cmd(&format!("LIST {path}"))?;
        if r.code == 550 {
            return Ok(Vec::new());
        }
        if r.code != 150 && r.code != 125 {
            return Err(format!("LIST {path} failed: {} {}", r.code, r.text));
        }
        let mut raw = String::new();
        data.read_to_string(&mut raw).map_err(|e| e.to_string())?;
        drop(data);
        self.read_reply()?;
        Ok(raw
            .lines()
            .filter_map(|l| {
                let parts: Vec<&str> = l.splitn(9, char::is_whitespace).filter(|p| !p.is_empty()).collect();
                // Unix-style listing: perms links owner group size month day time name
                let name = l.split_whitespace().skip(8).collect::<Vec<_>>().join(" ");
                if parts.is_empty() || name.is_empty() || name == "." || name == ".." {
                    return None;
                }
                Some((name, l.starts_with('d')))
            })
            .collect())
    }

    /// Uploads `src` to `path`, appending from `offset` when the server already has a prefix.
    pub fn store(
        &mut self,
        path: &str,
        src: &mut dyn Read,
        offset: u64,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), String> {
        let mut data = self.pasv()?;
        if offset > 0 {
            self.expect(&format!("REST {offset}"), &[350])?;
        }
        let r = self.cmd(&format!("STOR {path}"))?;
        if r.code != 150 && r.code != 125 {
            return Err(format!("Upload refused: {} {}", r.code, r.text));
        }
        let mut buf = vec![0u8; 1 << 20];
        let mut sent = offset;
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = data.shutdown(std::net::Shutdown::Both);
                let _ = self.read_reply();
                return Err("Cancelled".into());
            }
            let n = src.read(&mut buf).map_err(|e| format!("Reading the image failed: {e}"))?;
            if n == 0 {
                break;
            }
            data.write_all(&buf[..n]).map_err(|e| format!("Upload interrupted: {e}"))?;
            sent += n as u64;
            progress(sent);
        }
        data.flush().ok();
        drop(data);
        let done = self.read_reply()?;
        if done.code != 226 && done.code != 250 {
            return Err(format!("Upload failed: {} {}", done.code, done.text));
        }
        Ok(())
    }

    /// Writes a small file atomically: upload to a hidden temp name, then rename over.
    pub fn put_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let (dir, name) = path.rsplit_once('/').ok_or("bad path")?;
        let tmp = format!("{dir}/.{name}.packer-tmp");
        // ftpsrv's STOR doesn't truncate before writing, so never write over a leftover temp file.
        if self.size(&tmp)?.is_some() {
            self.delete(&tmp)?;
        }
        let never = AtomicBool::new(false);
        self.store(&tmp, &mut &bytes[..], 0, &never, &mut |_| {})?;
        self.rename(&tmp, path)
    }

    pub fn quit(mut self) {
        let _ = self.cmd("QUIT");
    }
}
