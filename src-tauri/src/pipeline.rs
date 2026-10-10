//! The job queue: compress → verify → upload → mount, one game at a time.

use crate::dump::DumpInfo;
use crate::engine::{Engine, PackEvent, VerifyEvent};
use crate::settings::{self, Settings};
use crate::{ps5, Emitter};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const STAGES: [&str; 4] = ["compress", "verify", "upload", "mount"];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    /// Verified, waiting for the user to start the upload.
    Ready,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StageState {
    Pending,
    Live,
    Done,
    Warn,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageInfo {
    pub key: &'static str,
    pub state: StageState,
    pub meta: String,
    pub pct: f64,
    pub secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub t: u64,
    pub text: String,
    /// "", "ok", "warn", "err", "hl"
    pub tone: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub kind: String,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: u64,
    pub dump: DumpInfo,
    pub status: JobStatus,
    pub stage: usize,
    pub stages: Vec<StageInfo>,
    /// Progress of the current stage, 0..100.
    pub pct: f64,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub files_done: u64,
    pub files_total: u64,
    /// Bytes per second, smoothed.
    pub rate: f64,
    pub rate_history: Vec<f64>,
    pub rate_label: &'static str,
    pub eta_secs: Option<u64>,
    pub stage_secs: u64,
    pub image_path: Option<String>,
    pub image_bytes: Option<u64>,
    pub remote_path: Option<String>,
    pub mismatches: u64,
    pub problems: Vec<Problem>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub log: Vec<LogLine>,
    #[serde(skip)]
    resume_from: usize,
}

impl Job {
    fn new(id: u64, dump: DumpInfo) -> Self {
        Self {
            id,
            dump,
            status: JobStatus::Queued,
            stage: 0,
            stages: STAGES
                .iter()
                .map(|k| StageInfo { key: k, state: StageState::Pending, meta: String::new(), pct: 0.0, secs: 0 })
                .collect(),
            pct: 0.0,
            done_bytes: 0,
            total_bytes: 0,
            files_done: 0,
            files_total: 0,
            rate: 0.0,
            rate_history: Vec::new(),
            rate_label: "",
            eta_secs: None,
            stage_secs: 0,
            image_path: None,
            image_bytes: None,
            remote_path: None,
            mismatches: 0,
            problems: Vec::new(),
            warnings: Vec::new(),
            error: None,
            log: Vec::new(),
            resume_from: 0,
        }
    }

    fn log(&mut self, tone: &'static str, text: impl Into<String>) {
        self.log.push(LogLine { t: now(), text: text.into(), tone });
        if self.log.len() > 500 {
            self.log.drain(..100);
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

struct State {
    jobs: Vec<Job>,
    next_id: u64,
    cancel: Option<(u64, Arc<AtomicBool>)>,
}

#[derive(Clone)]
pub struct Queue {
    inner: Arc<(Mutex<State>, Condvar)>,
    emitter: Emitter,
    engine: Arc<Engine>,
}

/// Smooths a byte counter into a rate and keeps a short history for the sparkline.
struct Meter {
    last_t: Instant,
    last_bytes: u64,
    last_sample: Instant,
    rate: f64,
}

impl Meter {
    fn new(start_bytes: u64) -> Self {
        let t = Instant::now();
        Self { last_t: t, last_bytes: start_bytes, last_sample: t, rate: 0.0 }
    }
    /// Returns Some(rate) once per ~second for the history graph.
    fn update(&mut self, bytes: u64) -> Option<f64> {
        let dt = self.last_t.elapsed().as_secs_f64();
        if dt >= 0.25 {
            let inst = bytes.saturating_sub(self.last_bytes) as f64 / dt;
            self.rate = if self.rate == 0.0 { inst } else { self.rate * 0.7 + inst * 0.3 };
            self.last_t = Instant::now();
            self.last_bytes = bytes;
        }
        if self.last_sample.elapsed() >= Duration::from_secs(1) {
            self.last_sample = Instant::now();
            return Some(self.rate);
        }
        None
    }
}

impl Queue {
    pub fn new(emitter: Emitter, engine: Arc<Engine>) -> Self {
        let q = Self {
            inner: Arc::new((Mutex::new(State { jobs: Vec::new(), next_id: 1, cancel: None }), Condvar::new())),
            emitter,
            engine,
        };
        let worker = q.clone();
        std::thread::Builder::new().name("packer-worker".into()).spawn(move || worker.run()).unwrap();
        q
    }

    pub fn snapshot(&self) -> Vec<Job> {
        self.inner.0.lock().unwrap().jobs.clone()
    }

    fn emit(&self) {
        let jobs = self.snapshot();
        (self.emitter)("jobs", serde_json::to_value(jobs).unwrap());
    }

    fn with<R>(&self, id: u64, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
        let mut st = self.inner.0.lock().unwrap();
        st.jobs.iter_mut().find(|j| j.id == id).map(f)
    }

    pub fn add(&self, dump: DumpInfo) -> Result<u64, String> {
        let id = {
            let mut st = self.inner.0.lock().unwrap();
            if let Some(j) = st.jobs.iter().find(|j| {
                j.dump.title_id == dump.title_id
                    && matches!(j.status, JobStatus::Queued | JobStatus::Running | JobStatus::Ready)
            }) {
                return Err(format!("{} is already in the queue", j.dump.title_name));
            }
            let id = st.next_id;
            st.next_id += 1;
            let mut job = Job::new(id, dump);
            job.log("hl", format!("Added {}", job.dump.path));
            if job.dump.apr && !job.dump.has_ampr_index {
                job.warnings.push(
                    "APR Emu game without ampr_emu.index: MkPFS will create that file inside the dump folder.".into(),
                );
            }
            st.jobs.push(job);
            id
        };
        self.inner.1.notify_all();
        self.emit();
        Ok(id)
    }

    pub fn remove(&self, id: u64) -> Result<(), String> {
        {
            let mut st = self.inner.0.lock().unwrap();
            let Some(pos) = st.jobs.iter().position(|j| j.id == id) else { return Ok(()) };
            if st.jobs[pos].status == JobStatus::Running {
                return Err("Cancel it first".into());
            }
            st.jobs.remove(pos);
        }
        self.emit();
        Ok(())
    }

    pub fn cancel(&self, id: u64) {
        let flag = {
            let mut st = self.inner.0.lock().unwrap();
            if let Some(j) = st.jobs.iter_mut().find(|j| j.id == id) {
                if matches!(j.status, JobStatus::Queued | JobStatus::Ready) {
                    j.status = JobStatus::Cancelled;
                    j.log("warn", "Cancelled");
                }
            }
            st.cancel.as_ref().filter(|(cid, _)| *cid == id).map(|(_, f)| f.clone())
        };
        if let Some(f) = flag {
            f.store(true, Ordering::Relaxed);
        }
        self.emit();
    }

    /// Re-queues a failed/cancelled job from the stage that didn't finish,
    /// or starts the upload of a verified job.
    pub fn resume(&self, id: u64) -> Result<(), String> {
        self.with(id, |j| {
            if !matches!(j.status, JobStatus::Failed | JobStatus::Cancelled | JobStatus::Ready) {
                return Err("Nothing to resume".to_string());
            }
            let mut from = j.stages.iter().position(|s| s.state != StageState::Done).unwrap_or(STAGES.len());
            // A failed verify means the image is bad: rebuild it rather than re-checking the same file.
            if j.status == JobStatus::Failed && from == 1 {
                if let Some(p) = j.image_path.take() {
                    if std::fs::remove_file(&p).is_ok() {
                        j.log("warn", format!("Deleted the image that failed verification: {p}"));
                    }
                }
                j.image_bytes = None;
                from = 0;
            }
            // Without the local image there is nothing to verify or upload.
            let from = if from > 0 && !j.image_path.as_ref().is_some_and(|p| Path::new(p).is_file()) { 0 } else { from };
            j.resume_from = from;
            j.status = JobStatus::Queued;
            j.error = None;
            for s in j.stages.iter_mut().skip(from) {
                s.state = StageState::Pending;
                s.pct = 0.0;
            }
            if j.status == JobStatus::Queued && from == 2 {
                j.log("hl", "Upload requested");
            }
            Ok(())
        })
        .unwrap_or(Err("Unknown job".into()))?;
        self.inner.1.notify_all();
        self.emit();
        Ok(())
    }

    fn run(self) {
        loop {
            let (id, cancel) = {
                let (lock, cv) = &*self.inner;
                let mut st = lock.lock().unwrap();
                loop {
                    if let Some(j) = st.jobs.iter_mut().find(|j| j.status == JobStatus::Queued) {
                        j.status = JobStatus::Running;
                        let id = j.id;
                        let flag = Arc::new(AtomicBool::new(false));
                        st.cancel = Some((id, flag.clone()));
                        break (id, flag);
                    }
                    st = cv.wait(st).unwrap();
                }
            };
            self.emit();
            self.process(id, &cancel);
            self.inner.0.lock().unwrap().cancel = None;
            self.emit();
        }
    }

    fn process(&self, id: u64, cancel: &Arc<AtomicBool>) {
        let s = settings::load();
        let from = self.with(id, |j| j.resume_from).unwrap_or(0);
        for stage in from..STAGES.len() {
            self.enter_stage(id, stage);
            let started = Instant::now();
            let result = match stage {
                0 => self.compress(id, &s, cancel),
                1 => self.verify(id, &s, cancel),
                2 => self.upload(id, &s, cancel),
                _ => self.mount(id, &s, cancel),
            };
            let secs = started.elapsed().as_secs();
            match result {
                Ok(()) => {
                    self.with(id, |j| {
                        let st = &mut j.stages[stage];
                        if st.state == StageState::Live {
                            st.state = StageState::Done;
                        }
                        st.pct = 100.0;
                        st.secs = secs;
                    });
                    if stage == 1 && !s.auto_upload {
                        self.with(id, |j| {
                            j.status = JobStatus::Ready;
                            j.resume_from = 2;
                            j.log("ok", "Verified. Waiting for you to start the upload.");
                        });
                        return;
                    }
                }
                Err(e) => {
                    let cancelled = cancel.load(Ordering::Relaxed);
                    self.with(id, |j| {
                        j.stages[stage].state = if cancelled { StageState::Pending } else { StageState::Failed };
                        j.stages[stage].secs = secs;
                        j.status = if cancelled { JobStatus::Cancelled } else { JobStatus::Failed };
                        j.rate = 0.0;
                        j.eta_secs = None;
                        if cancelled {
                            j.log("warn", "Cancelled");
                        } else {
                            j.log("err", e.clone());
                            j.error = Some(e);
                        }
                    });
                    return;
                }
            }
        }
        self.with(id, |j| {
            j.status = JobStatus::Done;
            j.rate = 0.0;
            j.eta_secs = None;
            j.stage = STAGES.len() - 1;
            j.log("ok", "Finished");
        });
    }

    fn enter_stage(&self, id: u64, stage: usize) {
        self.with(id, |j| {
            j.stage = stage;
            j.stages[stage].state = StageState::Live;
            j.stages[stage].pct = 0.0;
            j.pct = 0.0;
            j.done_bytes = 0;
            j.total_bytes = 0;
            j.files_done = 0;
            j.files_total = 0;
            j.rate = 0.0;
            j.rate_history.clear();
            j.eta_secs = None;
            j.stage_secs = 0;
            j.log("hl", format!("{} started", title_case(STAGES[stage])));
        });
        self.emit();
    }

    /// Shared progress bookkeeping; emits at most ~8 times a second.
    fn progress(&self, id: u64, last_emit: &mut Instant, started: Instant, f: impl FnOnce(&mut Job)) {
        self.with(id, |j| {
            f(j);
            j.stage_secs = started.elapsed().as_secs();
            j.stages[j.stage].pct = j.pct;
            if j.rate > 0.0 && j.total_bytes > j.done_bytes {
                j.eta_secs = Some(((j.total_bytes - j.done_bytes) as f64 / j.rate) as u64);
            }
        });
        if last_emit.elapsed() >= Duration::from_millis(120) {
            *last_emit = Instant::now();
            self.emit();
        }
    }

    // -- stages -------------------------------------------------------------

    fn compress(&self, id: u64, s: &Settings, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        let dump = self.with(id, |j| j.dump.clone()).unwrap();
        let out_dir = PathBuf::from(&s.output_dir);
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("Can't create {}: {e}", out_dir.display()))?;
        let final_path = out_dir.join(format!("{}.ffpfsc", dump.title_id));

        if final_path.is_file() {
            let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
            self.with(id, |j| {
                j.image_path = Some(final_path.to_string_lossy().into_owned());
                j.image_bytes = Some(size);
                j.stages[0].meta = format!("Reusing existing image · {}", gb(size));
                j.log("warn", format!("{} already exists; verifying it instead of compressing again", final_path.display()));
            });
            return Ok(());
        }

        let free = free_space(&out_dir).unwrap_or(u64::MAX);
        if free < dump.total_bytes / 3 {
            return Err(format!(
                "Only {} free in {}. The image could need up to {}.",
                gb(free),
                out_dir.display(),
                gb(dump.total_bytes)
            ));
        }
        if free < dump.total_bytes {
            self.with(id, |j| {
                j.warnings.push(format!(
                    "{} free in the output folder; that's enough only if the game compresses well.",
                    gb(free)
                ))
            });
        }

        let work = out_dir.join(format!(".work-{}", dump.title_id));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let work_img = work.join(format!("{}.ffpfsc", dump.title_id));

        let started = Instant::now();
        let mut last = Instant::now();
        let mut meter = Meter::new(0);
        let mut inner = dump.total_bytes;
        self.with(id, |j| {
            j.total_bytes = inner;
            j.rate_label = "compress";
            j.log("", format!("zlib level {} · writing to {}", s.compression_level, out_dir.display()));
        });
        let res = self.engine.pack(Path::new(&dump.path), &work_img, s.compression_level, s.cpu_count, cancel, &mut |ev| {
            match ev {
                PackEvent::InnerSize(n) => inner = n,
                PackEvent::Progress { pct, .. } => {
                    let done = (inner as f64 * pct / 100.0) as u64;
                    let sample = meter.update(done);
                    let rate = meter.rate;
                    self.progress(id, &mut last, started, |j| {
                        j.pct = pct;
                        j.total_bytes = inner;
                        j.done_bytes = done;
                        j.rate = rate;
                        if let Some(r) = sample {
                            push_hist(&mut j.rate_history, r);
                        }
                        j.stages[0].meta = format!("{:.0}% of {}", pct, gb(inner));
                    });
                }
                PackEvent::Line(l) => {
                    if is_interesting(&l) {
                        self.with(id, |j| j.log("", l));
                    }
                }
            }
        });
        if let Err(e) = res {
            let _ = std::fs::remove_dir_all(&work);
            return Err(e);
        }
        std::fs::rename(&work_img, &final_path).map_err(|e| format!("Couldn't move the image into place: {e}"))?;
        let _ = std::fs::remove_dir_all(&work);
        let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
        let secs = started.elapsed().as_secs();
        self.with(id, |j| {
            j.image_path = Some(final_path.to_string_lossy().into_owned());
            j.image_bytes = Some(size);
            let saved = j.dump.total_bytes.saturating_sub(size);
            j.stages[0].meta = format!("{} → {} · {}", gb(j.dump.total_bytes), gb(size), dur(secs));
            j.log(
                "ok",
                format!(
                    "Compressed to {} ({}% smaller) in {}",
                    gb(size),
                    (saved as f64 * 100.0 / j.dump.total_bytes.max(1) as f64).round(),
                    dur(secs)
                ),
            );
        });
        Ok(())
    }

    fn verify(&self, id: u64, s: &Settings, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        let (image, src) = self.with(id, |j| (j.image_path.clone(), j.dump.path.clone())).unwrap();
        let image = image.ok_or("No image to verify")?;
        let started = Instant::now();
        let mut last = Instant::now();
        let mut meter = Meter::new(0);
        self.with(id, |j| {
            j.mismatches = 0;
            j.problems.clear();
            j.rate_label = "verify";
            j.log("", "Comparing every file in the image with the dump, byte for byte");
        });
        let ok = self.engine.verify(Path::new(&image), Path::new(&src), s.verify_workers, cancel, &mut |ev| match ev {
            VerifyEvent::Plan { files, bytes } => {
                self.with(id, |j| {
                    j.files_total = files;
                    j.total_bytes = bytes;
                });
            }
            VerifyEvent::Progress { files, bytes } => {
                let sample = meter.update(bytes);
                let rate = meter.rate;
                self.progress(id, &mut last, started, |j| {
                    j.files_done = files;
                    j.done_bytes = bytes;
                    j.pct = if j.total_bytes > 0 { bytes as f64 * 100.0 / j.total_bytes as f64 } else { 0.0 };
                    j.rate = rate;
                    if let Some(r) = sample {
                        push_hist(&mut j.rate_history, r);
                    }
                    j.stages[1].meta = format!("{} of {} files · {} mismatches", fmt_n(files), fmt_n(j.files_total), j.mismatches);
                });
            }
            VerifyEvent::Problem { kind, path, detail } => {
                self.with(id, |j| {
                    j.mismatches += 1;
                    j.log("err", format!("{kind}: {path} ({detail})"));
                    if j.problems.len() < 200 {
                        j.problems.push(Problem { kind, path, detail });
                    }
                });
            }
            VerifyEvent::Done { .. } => {}
        })?;
        let secs = started.elapsed().as_secs();
        if !ok {
            let n = self.with(id, |j| j.mismatches).unwrap_or(0);
            return Err(format!("Verification failed: {n} problem(s). The image does not match the dump, so it won't be uploaded."));
        }
        self.with(id, |j| {
            j.pct = 100.0;
            j.stages[1].meta = format!("{} files · all match · {}", fmt_n(j.files_total), dur(secs));
            j.log("ok", format!("Verify passed: {} files, 0 mismatches", fmt_n(j.files_total)));
        });
        Ok(())
    }

    fn upload(&self, id: u64, s: &Settings, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        let (image, title) = self.with(id, |j| (j.image_path.clone(), j.dump.title_id.clone())).unwrap();
        let image = PathBuf::from(image.ok_or("No image to upload")?);
        let local_size = std::fs::metadata(&image).map_err(|e| format!("Image missing: {e}"))?.len();
        let dir = s.remote_dir.trim_end_matches('/').to_string();
        let final_remote = format!("{dir}/{title}.ffpfsc");
        let tmp_remote = format!("{dir}/.{title}.ffpfsc.packer-part");

        let mut ftp = ps5::connect(s)?;
        let others = ps5::competitors(&mut ftp, &dir, &title).unwrap_or_default();
        if !others.is_empty() {
            let msg = format!(
                "{dir} also has {}. ShadowMountPlus may mount that instead of the new image; remove it once the image works.",
                others.join(", ")
            );
            self.with(id, |j| {
                j.warnings.push(msg.clone());
                j.log("warn", msg);
            });
        }
        if let Some(existing) = ftp.size(&final_remote)? {
            if existing == local_size {
                self.with(id, |j| {
                    j.remote_path = Some(final_remote.clone());
                    j.stages[2].meta = "Already on the PS5".into();
                    j.log("ok", format!("{final_remote} is already on the PS5 with the same size; skipping upload"));
                });
                return Ok(());
            }
            return Err(format!(
                "The PS5 already has a different {title}.ffpfsc ({}). Delete or rename it first; Packer never overwrites.",
                gb(existing)
            ));
        }

        let started = Instant::now();
        let mut last = Instant::now();
        self.with(id, |j| {
            j.total_bytes = local_size;
            j.rate_label = "upload";
            j.log("", format!("{} → {}:{}", image.display(), s.ps5_host, final_remote));
        });

        let mut attempt = 0;
        loop {
            let offset = match ftp.size(&tmp_remote)? {
                Some(n) if n <= local_size => n,
                Some(_) => {
                    ftp.delete(&tmp_remote)?;
                    0
                }
                None => 0,
            };
            if offset >= local_size {
                break;
            }
            if offset > 0 {
                self.with(id, |j| j.log("", format!("Resuming at {}", gb(offset))));
            }
            let mut f = std::fs::File::open(&image).map_err(|e| e.to_string())?;
            std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
            let mut meter = Meter::new(offset);
            let res = ftp.store(&tmp_remote, &mut f, offset, cancel, &mut |sent| {
                let sample = meter.update(sent);
                let rate = meter.rate;
                self.progress(id, &mut last, started, |j| {
                    j.done_bytes = sent;
                    j.pct = sent as f64 * 100.0 / local_size.max(1) as f64;
                    j.rate = rate;
                    if let Some(r) = sample {
                        push_hist(&mut j.rate_history, r);
                    }
                    j.stages[2].meta = format!("{} of {}", gb(sent), gb(local_size));
                });
            });
            match res {
                Ok(()) => break,
                Err(e) if cancel.load(Ordering::Relaxed) => return Err(e),
                Err(e) => {
                    attempt += 1;
                    if attempt > 5 {
                        return Err(format!("{e} (gave up after 5 retries; Retry resumes where it stopped)"));
                    }
                    self.with(id, |j| j.log("warn", format!("{e}; reconnecting ({attempt}/5)")));
                    std::thread::sleep(Duration::from_secs(2 * attempt));
                    ftp = ps5::connect(s)?;
                }
            }
        }

        let remote_size = ftp.size(&tmp_remote)?.unwrap_or(0);
        if remote_size != local_size {
            return Err(format!(
                "Size check failed after upload: PS5 has {remote_size} bytes, image is {local_size}. Retry will resume."
            ));
        }
        ftp.rename(&tmp_remote, &final_remote)?;
        if ftp.size(&final_remote)? != Some(local_size) {
            return Err("The image was renamed but its size on the PS5 doesn't match".into());
        }
        ftp.quit();
        let secs = started.elapsed().as_secs();
        self.with(id, |j| {
            j.remote_path = Some(final_remote.clone());
            j.stages[2].meta = format!("{} · {}", gb(local_size), dur(secs));
            j.log("ok", format!("Uploaded {} in {} ({}/s)", gb(local_size), dur(secs), gb(local_size / secs.max(1))));
        });
        Ok(())
    }

    fn mount(&self, id: u64, s: &Settings, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        let (title, remote, image) =
            self.with(id, |j| (j.dump.title_id.clone(), j.remote_path.clone(), j.image_path.clone())).unwrap();
        let remote = remote.ok_or("Not uploaded yet")?;
        self.with(id, |j| j.rate_label = "");
        if s.write_smp_hints {
            let mut ftp = ps5::connect(s)?;
            match ps5::write_hints(&mut ftp, &title, &settings::data_dir().join("backups")) {
                Ok(r) => self.with(id, |j| {
                    if r.changed.is_empty() {
                        j.log("", "ShadowMountPlus settings already correct");
                    }
                    for c in r.changed {
                        j.log("", format!("ShadowMountPlus {c}"));
                    }
                    for b in r.backups {
                        j.log("", format!("Backup of the previous file: {b}"));
                    }
                }),
                Err(e) => self.with(id, |j| {
                    j.warnings.push(format!("Couldn't write ShadowMountPlus settings: {e}"));
                    j.log("warn", format!("Couldn't write ShadowMountPlus settings: {e}"));
                }),
            };
            ftp.quit();
        }
        let started = Instant::now();
        let mut last = Instant::now();
        let mounted = ps5::wait_for_mount(s, &title, &remote, Duration::from_secs(120), cancel, &mut |secs| {
            self.progress(id, &mut last, started, |j| {
                j.pct = (secs as f64 / 120.0 * 100.0).min(99.0);
                j.stages[3].meta = format!("Waiting for ShadowMountPlus · {secs}s");
            });
        })?;
        self.with(id, |j| {
            if mounted {
                j.stages[3].meta = "Mounted by ShadowMountPlus".into();
                j.log("ok", "ShadowMountPlus mounted the image. It's ready to play.");
            } else {
                j.stages[3].state = StageState::Warn;
                j.stages[3].meta = "Not confirmed within 2 minutes".into();
                j.warnings.push(
                    "ShadowMountPlus hasn't mounted the image yet. It scans every 15 s; check the PS5, or restart ShadowMountPlus."
                        .into(),
                );
                j.log("warn", "Mount not confirmed within 2 minutes");
            }
        });
        if mounted && s.delete_local_after_upload {
            if let Some(p) = image {
                if std::fs::remove_file(&p).is_ok() {
                    self.with(id, |j| j.log("", format!("Deleted local image {p}")));
                }
            }
        }
        Ok(())
    }
}

fn push_hist(h: &mut Vec<f64>, v: f64) {
    h.push(v);
    if h.len() > 90 {
        h.remove(0);
    }
}

fn is_interesting(l: &str) -> bool {
    let l = l.trim();
    !l.is_empty()
        && !l.starts_with("MkPFS ")
        && (l.starts_with("Wrapping")
            || l.starts_with("Inner image")
            || l.starts_with("Compressing")
            || l.contains("ampr")
            || l.contains("AMPR")
            || l.starts_with("Final image size")
            || l.starts_with("Actual gain")
            || l.starts_with("Throughput")
            || l.contains("❌")
            || l.contains("⚠")
            || l.to_ascii_lowercase().contains("error")
            || l.to_ascii_lowercase().contains("warning"))
}

fn free_space(p: &Path) -> Option<u64> {
    let c = std::ffi::CString::new(p.to_string_lossy().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statvfs(c.as_ptr(), &mut st) } == 0).then(|| st.f_bavail as u64 * st.f_frsize as u64)
}

pub fn gb(b: u64) -> String {
    let g = b as f64 / 1e9;
    if g >= 100.0 {
        format!("{g:.0} GB")
    } else if g >= 1.0 {
        format!("{g:.1} GB")
    } else {
        format!("{:.0} MB", b as f64 / 1e6)
    }
}

fn dur(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {:02}m", secs / 3600, secs % 3600 / 60)
    } else if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

fn fmt_n(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats() {
        assert_eq!(fmt_n(166733), "166,733");
        assert_eq!(fmt_n(12), "12");
        assert_eq!(gb(148_900_000_000), "149 GB");
        assert_eq!(gb(71_733_805_056), "71.7 GB");
        assert_eq!(dur(372), "6m 12s");
    }
}

/// End-to-end run against a local fake PS5 (FTP + fake ShadowMountPlus). Never touches a real
/// console. Run with: PACKER_E2E_SCRATCH=<dir with mkvenv, fake_ps5.py, fakedump> cargo test e2e -- --ignored
#[cfg(test)]
mod e2e {
    use super::*;

    fn wait_until(q: &Queue, id: u64, secs: u64) -> Job {
        let t = Instant::now();
        loop {
            let j = q.snapshot().into_iter().find(|j| j.id == id).unwrap();
            if matches!(j.status, JobStatus::Done | JobStatus::Failed | JobStatus::Cancelled | JobStatus::Ready) {
                return j;
            }
            assert!(t.elapsed().as_secs() < secs, "timed out in stage {}", j.stage);
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    #[test]
    #[ignore]
    fn e2e_local_fake_ps5() {
        let scratch = PathBuf::from(std::env::var("PACKER_E2E_SCRATCH").expect("PACKER_E2E_SCRATCH"));
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
        std::env::set_var("XDG_DATA_HOME", tmp.path().join("data"));
        let engine_dir = tmp.path().join("data/ps5-packer/engine");
        std::fs::create_dir_all(&engine_dir).unwrap();
        std::os::unix::fs::symlink(scratch.join("mkvenv"), engine_dir.join("venv")).unwrap();

        // Fresh copy of the fake dump so MkPFS's ampr_emu.index write can't leak between runs.
        let dump = tmp.path().join("PPSA00001-app");
        assert!(std::process::Command::new("cp")
            .args(["-r", scratch.join("fakedump/PPSA00001-app").to_str().unwrap(), dump.to_str().unwrap()])
            .status()
            .unwrap()
            .success());
        let _ = std::fs::remove_file(dump.join("ampr_emu.index"));

        let ps5_root = tmp.path().join("ps5");
        let port = 22121;
        let mut server = std::process::Command::new(scratch.join("mkvenv/bin/python"))
            .arg(scratch.join("fake_ps5.py"))
            .arg(&ps5_root)
            .arg(port.to_string())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(1500));

        let s = Settings {
            ps5_host: "127.0.0.1".into(),
            ftp_port: port,
            web_port: 1,
            output_dir: tmp.path().join("images").to_string_lossy().into_owned(),
            ..Settings::default()
        };
        settings::save(&s).unwrap();

        let engine = Arc::new(Engine::new(&settings::data_dir()));
        let q = Queue::new(Arc::new(|_, _| {}), engine);
        let id = q.add(crate::dump::inspect(&dump).unwrap()).unwrap();
        let j = wait_until(&q, id, 300);
        for l in &j.log {
            eprintln!("[{}] {}", l.tone, l.text);
        }
        assert_eq!(j.status, JobStatus::Done, "error: {:?}", j.error);
        assert!(j.stages.iter().all(|s| s.state == StageState::Done), "{:?}", j.stages);

        let local = std::fs::metadata(j.image_path.as_ref().unwrap()).unwrap().len();
        let remote = ps5_root.join("data/homebrew/PPSA00001.ffpfsc");
        assert_eq!(std::fs::metadata(&remote).unwrap().len(), local);
        assert!(!ps5_root.join("data/homebrew/.PPSA00001.ffpfsc.packer-part").exists());
        let cfg = std::fs::read_to_string(ps5_root.join("data/shadowmount/config.ini")).unwrap();
        assert!(cfg.contains("image_ro=OTHER.exfat") && cfg.contains("image_ro=PPSA00001.exfat"), "{cfg}");
        let at = std::fs::read_to_string(ps5_root.join("data/shadowmount/autotune.ini")).unwrap();
        assert_eq!(at, "image_sector=PPSA00001.ffpfsc:65536\n");
        assert!(dump.join("ampr_emu.index").is_file(), "MkPFS should have generated the AMPR index");

        // Same title again: image reused locally, upload skipped as identical.
        q.remove(id).unwrap();
        let id2 = q.add(crate::dump::inspect(&dump).unwrap()).unwrap();
        let j2 = wait_until(&q, id2, 300);
        assert_eq!(j2.status, JobStatus::Done, "error: {:?}", j2.error);
        assert!(j2.log.iter().any(|l| l.text.contains("already on the PS5")));

        // A tampered image must fail verification and never reach the upload.
        q.remove(id2).unwrap();
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut f = std::fs::OpenOptions::new().write(true).open(j.image_path.as_ref().unwrap()).unwrap();
            f.seek(SeekFrom::Start(local / 2)).unwrap();
            f.write_all(&[0xde, 0xad, 0xbe, 0xef, 0x00, 0xff]).unwrap();
        }
        std::fs::remove_file(&remote).unwrap();
        let id3 = q.add(crate::dump::inspect(&dump).unwrap()).unwrap();
        let j3 = wait_until(&q, id3, 300);
        assert_eq!(j3.status, JobStatus::Failed);
        assert_eq!(j3.stage, 1, "must stop at verify");
        assert!(j3.mismatches > 0);
        assert!(!remote.exists(), "a failed image must not be uploaded");

        // "Compress again" deletes the bad image, rebuilds, verifies and uploads.
        q.resume(id3).unwrap();
        let j4 = wait_until(&q, id3, 300);
        assert_eq!(j4.status, JobStatus::Done, "error: {:?}", j4.error);
        assert!(j4.log.iter().any(|l| l.text.contains("Deleted the image that failed verification")));

        // An interrupted upload resumes from the partial file instead of starting over.
        q.remove(id3).unwrap();
        let good = std::fs::read(j4.image_path.as_ref().unwrap()).unwrap();
        std::fs::remove_file(&remote).unwrap();
        let part = ps5_root.join("data/homebrew/.PPSA00001.ffpfsc.packer-part");
        std::fs::write(&part, &good[..good.len() / 2]).unwrap();
        let id5 = q.add(crate::dump::inspect(&dump).unwrap()).unwrap();
        let j5 = wait_until(&q, id5, 300);
        assert_eq!(j5.status, JobStatus::Done, "error: {:?}", j5.error);
        assert!(j5.log.iter().any(|l| l.text.starts_with("Resuming at")), "should resume");
        assert_eq!(std::fs::read(&remote).unwrap(), good, "resumed upload must be byte-identical");
        assert!(!part.exists());

        server.kill().ok();
    }
}
