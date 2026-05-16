//! TUI driver: spawn the orchestrator on a background thread, render
//! state on the main thread.
//!
//! The TUI is a *sink* for the orchestrator's `FlowEvent` stream
//! (see [`TuiSink`]). It does not import any of the orchestrator's
//! types beyond `FlowEvent` and workflow entry points — adding new
//! steps or reordering them in `add_card.rs` does not require any
//! change here.

use std::io::{Read, Seek, SeekFrom, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::add_card;
use crate::flow::{FlowEvent, FlowSink, NoteLevel};
use crate::grind;
use crate::refactor_hotspot;

mod complexity;
mod input;
mod state;
mod view;

pub use state::AppState;

const HOT_RELOAD_EXIT_CODE: u8 = 75;

enum ViewerExit {
    Quit,
    Reload,
}

/// Sink that pushes events into a channel for the TUI to consume.
pub struct TuiSink {
    tx: Sender<FlowEvent>,
    stop_requested: Arc<AtomicBool>,
}

impl FlowSink for TuiSink {
    fn emit(&mut self, event: FlowEvent) {
        // If the TUI has shut down the channel, swallow the event —
        // there's no useful action the orchestrator could take here.
        let _ = self.tx.send(event);
    }

    fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }
}

struct EventLogSink {
    file: std::fs::File,
    stop_request_path: PathBuf,
}

impl FlowSink for EventLogSink {
    fn emit(&mut self, event: FlowEvent) {
        if serde_json::to_writer(&mut self.file, &event).is_ok() {
            let _ = writeln!(self.file);
            let _ = self.file.flush();
        }
    }

    fn stop_requested(&self) -> bool {
        self.stop_request_path.exists()
    }
}

/// Run add-card with the TUI as its output surface.
pub fn run_add_card(opts: add_card::Options) -> Result<std::process::ExitCode> {
    run_workflow(move |sink| add_card::run_with_sink(opts, sink))
}

pub fn run_add_card_hot_reload(opts: add_card::Options) -> Result<std::process::ExitCode> {
    run_workflow_hot_reload("add-card", move |sink| add_card::run_with_sink(opts, sink))
}

/// Run refactor-hotspot with the TUI as its output surface.
pub fn run_refactor_hotspot(opts: refactor_hotspot::Options) -> Result<std::process::ExitCode> {
    run_workflow(move |sink| refactor_hotspot::run_with_sink(opts, sink))
}

pub fn run_refactor_hotspot_hot_reload(
    opts: refactor_hotspot::Options,
) -> Result<std::process::ExitCode> {
    run_workflow_hot_reload("refactor-hotspot", move |sink| {
        refactor_hotspot::run_with_sink(opts, sink)
    })
}

/// Run grind with the TUI as its output surface. The two inner workflows
/// (refactor-hotspot, then add-card) reuse the same FlowEvent stream;
/// the TUI re-renders as the SessionStarted events from each phase land.
pub fn run_grind(opts: grind::Options) -> Result<std::process::ExitCode> {
    run_workflow(move |sink| grind::run_with_sink(opts, sink))
}

pub fn run_grind_hot_reload(opts: grind::Options) -> Result<std::process::ExitCode> {
    run_workflow_hot_reload("grind", move |sink| grind::run_with_sink(opts, sink))
}

pub fn run_viewer(event_log: PathBuf) -> Result<std::process::ExitCode> {
    let mut terminal = setup_terminal().context("set up terminal")?;
    terminal.clear()?;
    let result = run_event_loop_from_log(&mut terminal, &event_log);
    teardown_terminal(&mut terminal).ok();
    Ok(match result? {
        ViewerExit::Quit => std::process::ExitCode::SUCCESS,
        ViewerExit::Reload => std::process::ExitCode::from(HOT_RELOAD_EXIT_CODE),
    })
}

fn run_workflow<F>(orchestrator: F) -> Result<std::process::ExitCode>
where
    F: FnOnce(&mut dyn FlowSink) -> Result<std::process::ExitCode> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<FlowEvent>();
    let stop_requested = Arc::new(AtomicBool::new(false));

    // Orchestrator on background thread; sink owned by the thread.
    let orchestrator_stop_requested = Arc::clone(&stop_requested);
    let orchestrator_handle = thread::spawn(move || -> Result<std::process::ExitCode> {
        let mut sink: Box<dyn FlowSink> = Box::new(TuiSink {
            tx,
            stop_requested: orchestrator_stop_requested,
        });
        match orchestrator(sink.as_mut()) {
            Ok(code) => Ok(code),
            Err(err) => {
                let reason = format!("{err:#}");
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Error,
                    text: format!("startup failed: {reason}"),
                });
                sink.emit(FlowEvent::SessionFinished {
                    reason: crate::flow::SessionEndReason::SurfacedToHuman(reason),
                });
                Ok(std::process::ExitCode::FAILURE)
            }
        }
    });

    // TUI on the main thread.
    let mut terminal = setup_terminal().context("set up terminal")?;
    let tui_result = run_event_loop(&mut terminal, rx, stop_requested);
    teardown_terminal(&mut terminal).ok();

    // Wait for orchestrator to finish (it will after the user quits or
    // it completes its loop). Errors from the orchestrator are surfaced
    // after teardown so they get a normal stderr line.
    let orchestrator_result = orchestrator_handle
        .join()
        .map_err(|_| anyhow::anyhow!("orchestrator thread panicked"))?;

    match (tui_result, orchestrator_result) {
        (Err(e), _) | (_, Err(e)) => Err(e),
        (Ok(()), Ok(code)) => Ok(code),
    }
}

fn run_workflow_hot_reload<F>(
    workflow: &'static str,
    orchestrator: F,
) -> Result<std::process::ExitCode>
where
    F: FnOnce(&mut dyn FlowSink) -> Result<std::process::ExitCode> + Send + 'static,
{
    let event_log = hot_reload_event_log_path(workflow);
    let stop_request_path = hot_reload_stop_request_path(&event_log);
    let _ = std::fs::remove_file(&stop_request_path);
    if let Some(parent) = event_log.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(&event_log)
        .with_context(|| format!("create TUI hot-reload event log {}", event_log.display()))?;

    let orchestrator_handle = thread::spawn(move || -> Result<std::process::ExitCode> {
        let mut sink: Box<dyn FlowSink> = Box::new(EventLogSink {
            file,
            stop_request_path,
        });
        match orchestrator(sink.as_mut()) {
            Ok(code) => Ok(code),
            Err(err) => {
                let reason = format!("{err:#}");
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Error,
                    text: format!("startup failed: {reason}"),
                });
                sink.emit(FlowEvent::SessionFinished {
                    reason: crate::flow::SessionEndReason::SurfacedToHuman(reason),
                });
                Ok(std::process::ExitCode::FAILURE)
            }
        }
    });

    let mut child = spawn_hot_reload_viewer(&event_log)?;
    loop {
        let status = child.wait()?;
        if status.code() == Some(HOT_RELOAD_EXIT_CODE as i32) {
            child = spawn_hot_reload_viewer(&event_log)?;
            continue;
        }
        break;
    }

    orchestrator_handle
        .join()
        .map_err(|_| anyhow::anyhow!("orchestrator thread panicked"))?
}

fn hot_reload_event_log_path(workflow: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mtg-parser-{workflow}-tui-{}.ndjson",
        std::process::id()
    ))
}

fn hot_reload_stop_request_path(event_log: &Path) -> PathBuf {
    event_log.with_extension("stop")
}

fn spawn_hot_reload_viewer(event_log: &Path) -> Result<Child> {
    Command::new("cargo")
        .args(["run", "-q", "-p", "xtask", "--", "tui-view", "--event-log"])
        .arg(event_log)
        .spawn()
        .context("spawn hot-reload TUI viewer")
}

fn tui_source_mtime() -> Result<SystemTime> {
    let dir = crate::paths::repo_root().join("xtask/src/tui");
    let mut newest = SystemTime::UNIX_EPOCH;
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        newest = newest.max(modified);
    }
    Ok(newest)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    rx: Receiver<FlowEvent>,
    stop_requested: Arc<AtomicBool>,
) -> Result<()> {
    let mut state = AppState::new();
    let poll_timeout = Duration::from_millis(50);
    let mut last_area = terminal.size()?;

    loop {
        // 1. Drain any pending events from the orchestrator.
        let mut closed = false;
        loop {
            match rx.try_recv() {
                Ok(ev) => {
                    let hard_redraw = matches!(ev, FlowEvent::IterationStarted { .. });
                    state.apply(ev);
                    if hard_redraw {
                        terminal.autoresize()?;
                        terminal.clear()?;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state.orchestrator_done = true;
                    closed = true;
                    break;
                }
            }
        }
        let _ = closed;

        // 2. Render.
        terminal.autoresize()?;
        let area = terminal.size()?;
        if area != last_area {
            terminal.clear()?;
            last_area = area;
        }
        terminal.draw(|f| view::render(f, &mut state))?;

        // 3. Poll input. Short timeout so we redraw on a regular cadence
        //    (clock ticks for running step timers etc.).
        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key) => match input::handle(key, &mut state) {
                    input::Action::Quit => break,
                    input::Action::StopAfterCurrent => {
                        stop_requested.store(true, Ordering::Relaxed);
                    }
                    input::Action::YankVisual => yank_visual_to_clipboard(&mut state),
                    input::Action::OpenCard => open_active_card(&mut state),
                    input::Action::None => {}
                },
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    terminal.clear()?;
                }
                Event::Mouse(mouse) => match input::handle_mouse(mouse, &mut state) {
                    input::Action::Quit => break,
                    input::Action::StopAfterCurrent => {
                        stop_requested.store(true, Ordering::Relaxed);
                    }
                    input::Action::YankVisual => yank_visual_to_clipboard(&mut state),
                    input::Action::OpenCard => open_active_card(&mut state),
                    input::Action::None => {}
                },
                _ => {}
            }
        }
    }
    Ok(())
}

fn run_event_loop_from_log(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    event_log: &Path,
) -> Result<ViewerExit> {
    let mut state = AppState::new();
    let mut reader = EventLogReader::new(event_log.to_path_buf());
    let stop_request_path = hot_reload_stop_request_path(event_log);
    let poll_timeout = Duration::from_millis(50);
    let mut last_area = terminal.size()?;
    let mut last_ui_mtime = tui_source_mtime()?;

    loop {
        let current_mtime = tui_source_mtime()?;
        if current_mtime > last_ui_mtime {
            return Ok(ViewerExit::Reload);
        }
        last_ui_mtime = current_mtime;

        for ev in reader.read_available()? {
            let hard_redraw = matches!(ev, FlowEvent::IterationStarted { .. });
            state.apply(ev);
            if hard_redraw {
                terminal.autoresize()?;
                terminal.clear()?;
            }
        }
        if stop_request_path.exists() {
            state.stop_after_current = true;
        }

        terminal.autoresize()?;
        let area = terminal.size()?;
        if area != last_area {
            terminal.clear()?;
            last_area = area;
        }
        terminal.draw(|f| view::render(f, &mut state))?;

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key) => match input::handle(key, &mut state) {
                    input::Action::Quit => return Ok(ViewerExit::Quit),
                    input::Action::StopAfterCurrent => {
                        std::fs::write(&stop_request_path, b"stop after current iteration")?;
                    }
                    input::Action::YankVisual => yank_visual_to_clipboard(&mut state),
                    input::Action::OpenCard => open_active_card(&mut state),
                    input::Action::None => {}
                },
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    terminal.clear()?;
                }
                Event::Mouse(mouse) => match input::handle_mouse(mouse, &mut state) {
                    input::Action::Quit => return Ok(ViewerExit::Quit),
                    input::Action::StopAfterCurrent => {
                        std::fs::write(&stop_request_path, b"stop after current iteration")?;
                    }
                    input::Action::YankVisual => yank_visual_to_clipboard(&mut state),
                    input::Action::OpenCard => open_active_card(&mut state),
                    input::Action::None => {}
                },
                _ => {}
            }
        }
    }
}

struct EventLogReader {
    path: PathBuf,
    offset: u64,
    partial: String,
}

impl EventLogReader {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial: String::new(),
        }
    }

    fn read_available(&mut self) -> Result<Vec<FlowEvent>> {
        let mut file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        file.seek(SeekFrom::Start(self.offset))?;
        let mut chunk = String::new();
        file.read_to_string(&mut chunk)?;
        self.offset += chunk.len() as u64;
        if chunk.is_empty() {
            return Ok(Vec::new());
        }

        self.partial.push_str(&chunk);
        let complete = self.partial.ends_with('\n');
        let mut lines = self.partial.lines().map(str::to_string).collect::<Vec<_>>();
        if !complete {
            self.partial = lines.pop().unwrap_or_default();
        } else {
            self.partial.clear();
        }

        let mut events = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(&line)?);
        }
        Ok(events)
    }
}

fn open_active_card(state: &mut AppState) {
    let Some(card) = state.active_iteration().and_then(|iter| iter.card.as_ref()) else {
        return;
    };
    let url = view::scryfall_card_url(&card.name);
    let result = open_url(&url);
    match result {
        Ok(()) => state.push_ui_note(format!("opened {}", card.name)),
        Err(err) => state.events.push(state::TimelineRow {
            iteration_index: state.iterations.len() as u32,
            delta: 0,
            kind: state::TimelineKind::Note {
                level: NoteLevel::Warn,
                text: format!("open failed: {err}; {url}"),
            },
        }),
    }
}

fn open_url(url: &str) -> Result<()> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("open", &[])]
    } else if cfg!(target_os = "linux") {
        &[("xdg-open", &[])]
    } else {
        &[]
    };
    for (program, args) in candidates {
        let status = Command::new(program).args(*args).arg(url).status();
        match status {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => anyhow::bail!("{program} exited with {status}"),
            Err(err) => anyhow::bail!("{program}: {err}"),
        }
    }
    anyhow::bail!("no browser opener configured for this platform")
}

fn yank_visual_to_clipboard(state: &mut AppState) {
    let text = state.visual_text();
    let result = copy_to_clipboard(&text);
    state.visual.cancel();
    match result {
        Ok(()) => state.push_ui_note(format!(
            "yanked {} line(s) to clipboard",
            text.lines().count()
        )),
        Err(err) => state.events.push(state::TimelineRow {
            iteration_index: state.iterations.len() as u32,
            delta: 0,
            kind: state::TimelineKind::Note {
                level: NoteLevel::Warn,
                text: format!("copy failed: {err}"),
            },
        }),
    }
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "linux") {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    } else {
        &[]
    };

    let mut errors = Vec::new();
    for (program, args) in candidates {
        match run_clipboard_command(program, args, text) {
            Ok(()) => return Ok(()),
            Err(err) => errors.push(format!("{program}: {err}")),
        }
    }

    if candidates.is_empty() {
        anyhow::bail!("no clipboard command configured for this platform");
    }
    anyhow::bail!("{}", errors.join("; "))
}

fn run_clipboard_command(program: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn {program}"))?;

    child
        .stdin
        .as_mut()
        .context("clipboard command did not open stdin")?
        .write_all(text.as_bytes())
        .with_context(|| format!("write to {program}"))?;

    let status = child
        .wait()
        .with_context(|| format!("wait for {program}"))?;
    if !status.success() {
        anyhow::bail!("exited with {status}");
    }
    Ok(())
}
