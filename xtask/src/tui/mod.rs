//! TUI driver: spawn the orchestrator on a background thread, render
//! state on the main thread.
//!
//! The TUI is a *sink* for the orchestrator's `FlowEvent` stream
//! (see [`TuiSink`]). It does not import any of the orchestrator's
//! types beyond `FlowEvent` and `grammar_fix::Options` — adding new
//! steps or reordering them in `grammar_fix.rs` does not require any
//! change here.

use std::io::{Stdout, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::flow::{FlowEvent, FlowSink, NoteLevel};
use crate::grammar_fix;

mod input;
mod state;
mod view;

pub use state::AppState;

/// Sink that pushes events into a channel for the TUI to consume.
pub struct TuiSink {
    tx: Sender<FlowEvent>,
}

impl FlowSink for TuiSink {
    fn emit(&mut self, event: FlowEvent) {
        // If the TUI has shut down the channel, swallow the event —
        // there's no useful action the orchestrator could take here.
        let _ = self.tx.send(event);
    }
}

/// Run grammar-fix with the TUI as its output surface.
pub fn run(opts: grammar_fix::Options) -> Result<std::process::ExitCode> {
    let (tx, rx) = mpsc::channel::<FlowEvent>();

    // Orchestrator on background thread; sink owned by the thread.
    let orchestrator_handle = thread::spawn(move || -> Result<std::process::ExitCode> {
        let mut sink: Box<dyn FlowSink> = Box::new(TuiSink { tx });
        match grammar_fix::run_with_sink(opts, sink.as_mut()) {
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
    let tui_result = run_event_loop(&mut terminal, rx);
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
) -> Result<()> {
    let mut state = AppState::new();
    let poll_timeout = Duration::from_millis(50);

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
        terminal.draw(|f| view::render(f, &mut state))?;

        // 3. Poll input. Short timeout so we redraw on a regular cadence
        //    (clock ticks for running step timers etc.).
        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key) => match input::handle(key, &mut state) {
                    input::Action::Quit => break,
                    input::Action::Copy(target) => copy_target_to_clipboard(&mut state, target),
                    input::Action::None => {}
                },
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    terminal.clear()?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn copy_target_to_clipboard(state: &mut AppState, target: input::CopyTarget) {
    let (label, text) = match target {
        input::CopyTarget::Output => ("output", state.output_text()),
        input::CopyTarget::Card => ("card", state.card_text()),
        input::CopyTarget::Steps => ("steps", state.steps_text()),
        input::CopyTarget::All => ("all", state.all_json_text()),
        input::CopyTarget::Visual => ("visual", state.visual_text()),
    };
    let result = copy_to_clipboard(&text);
    if matches!(target, input::CopyTarget::Visual) {
        state.visual.cancel();
    }
    match result {
        Ok(()) => state.push_ui_note(format!(
            "copied {label} ({} line(s)) to clipboard",
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
