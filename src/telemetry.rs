//! Opt-in frame telemetry for performance work.
//!
//! `--telemetry[=FRAMES]` runs the chosen scene for a fixed number of frames,
//! recording the wall-clock time of each phase of the frame loop, how long the
//! loop spent blocked in `next_frame`, and, for one sampled frame, the batches
//! macroquad flushed to the GPU. A summary is printed to stdout on exit.
//!
//! The phases are measured where the frame loop hands control over, so they
//! include whatever macroquad does at that boundary. Switching cameras flushes
//! the pending batches to the driver, which means GL submission cost lands in
//! the phase that switches cameras, not the phase that queued the geometry:
//! the frame is drawn through one camera, so that is the switch back to the
//! screen in `present`.
//!
//! `--gpu-sync` additionally forces the GPU to finish each frame's work before
//! the next frame starts and records how long that took. macOS OpenGL has no
//! timer queries, so this is the only in-process bound on GPU time: it is the
//! part of the frame's GPU work that did not overlap with CPU submission.

use macroquad::miniquad;
use macroquad::prelude::*;
use std::time::{Duration, Instant};

/// Frames recorded when `--telemetry` is given without a count.
const DEFAULT_FRAMES: usize = 600;

/// Frames discarded at the start of a run, so pipeline creation, first-use
/// buffer allocation and texture uploads do not skew the percentiles.
const WARMUP_FRAMES: usize = 30;

/// Frame (counted from the first recorded frame) during which the batch
/// capture is requested. macroquad captures the frame after the request and
/// re-renders every batch into a scratch texture while doing so, which
/// inflates that frame's timings, so the captured frame and the one whose
/// interval it distorts are excluded from the statistics.
const CAPTURE_REQUEST_FRAME: usize = 10;

/// A batch this small is a sign the batcher was broken mid-object: a single
/// quad is 6 indices, a cube's visible edges are a dozen.
const SMALL_BATCH_INDICES: usize = 12;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Input sampling and `GameState::update`.
    Update,
    /// Everything queued for the low-resolution render target: geometry
    /// generation, since nothing reaches the driver until `Present`.
    Draw,
    /// Submitting the frame's batches to the driver, then the present blit.
    Present,
    /// Only with `--gpu-sync`: a forced flush plus `glFinish`.
    GpuSync,
    /// `next_frame().await`: macroquad's end-of-frame flush and buffer swap,
    /// the display-link wait and event processing.
    Wait,
}

const PHASES: [Phase; 5] = [
    Phase::Update,
    Phase::Draw,
    Phase::Present,
    Phase::GpuSync,
    Phase::Wait,
];

impl Phase {
    fn index(self) -> usize {
        PHASES.iter().position(|&phase| phase == self).unwrap()
    }

    fn label(self) -> &'static str {
        match self {
            Phase::Update => "update",
            Phase::Draw => "draw",
            Phase::Present => "present",
            Phase::GpuSync => "gpu_sync",
            Phase::Wait => "wait",
        }
    }
}

#[derive(Copy, Clone, Default)]
struct FrameSample {
    /// Time from the previous frame's start to this one's: the pacing the
    /// player actually sees.
    interval: Duration,
    phases: [Duration; PHASES.len()],
}

impl FrameSample {
    /// Time the loop spent doing work rather than waiting for the next frame.
    fn busy(&self) -> Duration {
        self.phases[Phase::Update.index()]
            + self.phases[Phase::Draw.index()]
            + self.phases[Phase::Present.index()]
    }
}

/// What macroquad flushed for the captured frame.
struct BatchCapture {
    call_count: usize,
    index_total: usize,
    small_call_count: usize,
    texture_count: usize,
}

/// Number of frames requested by `--telemetry[=FRAMES]`, if it was given.
pub fn frames_from_args() -> Option<usize> {
    std::env::args().find_map(|arg| {
        if arg == "--telemetry" {
            return Some(DEFAULT_FRAMES);
        }

        arg.strip_prefix("--telemetry=")
            .and_then(|count| count.parse().ok())
    })
}

pub struct Telemetry {
    /// Unset when telemetry was not requested; every method is then a no-op
    /// so the frame loop can call them unconditionally.
    enabled: bool,
    scene_label: String,
    frames_to_record: usize,
    gpu_sync: bool,
    frames_seen: usize,
    frame_start: Option<Instant>,
    phase_start: Option<(Phase, Instant)>,
    current: FrameSample,
    samples: Vec<FrameSample>,
    batches: Option<BatchCapture>,
}

impl Telemetry {
    /// Build a recorder from the command line. Without `--telemetry` the
    /// recorder is disabled and costs one branch per call.
    pub fn from_args(scene_label: &str) -> Self {
        let frames_to_record = frames_from_args();

        Self {
            enabled: frames_to_record.is_some(),
            scene_label: scene_label.to_owned(),
            frames_to_record: frames_to_record.unwrap_or(0),
            gpu_sync: std::env::args().any(|arg| arg == "--gpu-sync"),
            frames_seen: 0,
            frame_start: None,
            phase_start: None,
            current: FrameSample::default(),
            samples: Vec::with_capacity(frames_to_record.unwrap_or(0)),
            batches: None,
        }
    }

    /// Close out the previous frame and start timing a new one in the
    /// `Update` phase. Returns `true` once enough frames have been recorded
    /// for the caller to print the report and quit.
    pub fn begin_frame(&mut self) -> bool {
        if !self.enabled {
            return false;
        }

        let now = Instant::now();
        self.close_phase(now);

        if let Some(previous_start) = self.frame_start {
            self.current.interval = now - previous_start;
            self.record_frame();
        }

        self.frame_start = Some(now);
        self.current = FrameSample::default();
        self.phase_start = Some((Phase::Update, now));
        self.frames_seen += 1;

        self.schedule_batch_capture();

        self.samples.len() >= self.frames_to_record
    }

    /// Attribute the time since the last phase change to that phase and
    /// start timing `phase`.
    pub fn enter(&mut self, phase: Phase) {
        if !self.enabled {
            return;
        }

        let now = Instant::now();
        self.close_phase(now);
        self.phase_start = Some((phase, now));
    }

    /// With `--gpu-sync`, flush everything queued so far and block until the
    /// GPU has executed it. Without it this is a plain transition to `Wait`.
    pub fn sync_gpu_then_wait(&mut self) {
        if !self.enabled {
            return;
        }

        if self.gpu_sync {
            self.enter(Phase::GpuSync);
            set_default_camera();
            unsafe {
                miniquad::gl::glFinish();
            }
        }

        self.enter(Phase::Wait);
    }

    fn close_phase(&mut self, now: Instant) {
        if let Some((phase, started)) = self.phase_start.take() {
            self.current.phases[phase.index()] += now - started;
        }
    }

    fn recorded_frame_index(&self) -> Option<usize> {
        self.frames_seen.checked_sub(WARMUP_FRAMES + 1)
    }

    fn record_frame(&mut self) {
        let Some(index) = self.recorded_frame_index() else {
            return;
        };

        // The captured frame is the one after the request; its own timings
        // and the interval of the frame after it carry the capture overhead.
        let tainted = index == CAPTURE_REQUEST_FRAME + 1 || index == CAPTURE_REQUEST_FRAME + 2;
        if tainted {
            return;
        }

        self.samples.push(self.current);
    }

    fn schedule_batch_capture(&mut self) {
        let Some(index) = self.recorded_frame_index() else {
            return;
        };

        if index == CAPTURE_REQUEST_FRAME {
            macroquad::telemetry::capture_frame();
        }

        if index == CAPTURE_REQUEST_FRAME + 2 && self.batches.is_none() {
            let calls = macroquad::telemetry::drawcalls();
            self.batches = Some(BatchCapture {
                call_count: calls.len(),
                index_total: calls.iter().map(|call| call.indices_count).sum(),
                small_call_count: calls
                    .iter()
                    .filter(|call| call.indices_count <= SMALL_BATCH_INDICES)
                    .count(),
                texture_count: macroquad::telemetry::textures_count(),
            });
        }
    }

    /// Print the summary of the recorded frames to stdout.
    pub fn report(&self) {
        let intervals: Vec<f64> = self.samples.iter().map(|s| ms(s.interval)).collect();
        let median_interval = percentile(&intervals, 50.0);
        let hitches = intervals
            .iter()
            .filter(|&&interval| interval > median_interval * 1.5)
            .count();
        let busy: Vec<f64> = self.samples.iter().map(|s| ms(s.busy())).collect();

        println!(
            "telemetry scene={} frames={} warmup={} gpu_sync={}",
            self.scene_label,
            self.samples.len(),
            WARMUP_FRAMES,
            if self.gpu_sync { "on" } else { "off" },
        );
        println!(
            "frame interval  {}  ~{:.1} Hz  hitches(>1.5x median)={}",
            stats_line(&intervals),
            1000.0 / median_interval,
            hitches,
        );
        println!("cpu busy        {}", stats_line(&busy));

        for phase in PHASES {
            if phase == Phase::GpuSync && !self.gpu_sync {
                continue;
            }

            let values: Vec<f64> = self
                .samples
                .iter()
                .map(|s| ms(s.phases[phase.index()]))
                .collect();
            println!("  {:<13} {}", phase.label(), stats_line(&values));
        }

        match &self.batches {
            Some(batches) => println!(
                "batches         {} draw calls, {} indices, {} calls with <= {} indices, {} textures",
                batches.call_count,
                batches.index_total,
                batches.small_call_count,
                SMALL_BATCH_INDICES,
                batches.texture_count,
            ),
            None => println!("batches         not captured (run was too short)"),
        }
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn percentile(values: &[f64], percent: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = (percent / 100.0 * (sorted.len() - 1) as f64).round() as usize;

    sorted[rank.min(sorted.len() - 1)]
}

fn stats_line(values: &[f64]) -> String {
    if values.is_empty() {
        return String::from("no samples");
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let max = values.iter().copied().fold(f64::MIN, f64::max);

    format!(
        "mean {:>6.3}  p50 {:>6.3}  p95 {:>6.3}  p99 {:>6.3}  max {:>6.3} ms",
        mean,
        percentile(values, 50.0),
        percentile(values, 95.0),
        percentile(values, 99.0),
        max,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_picks_nearest_rank() {
        let values = [5.0, 1.0, 3.0, 2.0, 4.0];

        assert_eq!(percentile(&values, 0.0), 1.0);
        assert_eq!(percentile(&values, 50.0), 3.0);
        assert_eq!(percentile(&values, 100.0), 5.0);
        assert_eq!(percentile(&[], 50.0), 0.0);
    }

    #[test]
    fn busy_time_excludes_the_wait_for_the_next_frame() {
        let mut sample = FrameSample::default();
        sample.phases[Phase::Update.index()] = Duration::from_millis(1);
        sample.phases[Phase::Draw.index()] = Duration::from_millis(2);
        sample.phases[Phase::Present.index()] = Duration::from_millis(3);
        sample.phases[Phase::Wait.index()] = Duration::from_millis(10);

        assert_eq!(sample.busy(), Duration::from_millis(6));
    }
}
