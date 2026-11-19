use amo_vs::app::App;
use amo_vs::platform::terminal_restore_sequence;
use amo_vs::services::Config as Settings;
use amo_vs::themes::load as load_theme;
use amo_vs::ui;
use crossterm::event::{
    EventStream, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture, Event},
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static MOUSE_ENABLED: AtomicBool = AtomicBool::new(true);
static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

fn print_help() {
    println!("AMO VS - A VS Code-inspired terminal code editor\n");
    println!("Usage: amo-vs [OPTIONS] [PATH]\n");
    println!("Arguments:");
    println!("  PATH            File or folder to open on startup\n");
    println!("Options:");
    println!("  -h, --help      Print help information");
    println!("  -V, --version   Print version information");
    println!("  --no-mouse      Disable mouse capture");
    println!("  --config DIR    Use a custom configuration directory");
    println!("\nExit: Ctrl+Esc (or Ctrl+Q) from any panel.");
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    // Panic hook: never leave the user's terminal in raw mode / alt screen.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if TERMINAL_ACTIVE.load(Ordering::SeqCst) {
            let mut out = io::stdout();
            let _ = out.write_all(&terminal_restore_sequence(MOUSE_ENABLED.load(Ordering::Relaxed)));
            let _ = out.flush();
            let _ = disable_raw_mode();
            TERMINAL_ACTIVE.store(false, Ordering::SeqCst);
        }
        default_hook(info);
    }));

    let mut args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut mouse = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => { print_help(); return Ok(()); }
            "-V" | "--version" => {
                println!("AMO VS {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--no-mouse" => mouse = false,
            "--config" => {
                if let Some(dir) = args.next() {
                    std::env::set_var("XDG_CONFIG_HOME", dir);
                }
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                print_help();
                return Ok(());
            }
            _ => path = Some(PathBuf::from(arg)),
        }
    }

    MOUSE_ENABLED.store(mouse, Ordering::Relaxed);
    Settings::ensure_default_file();
    let config = Settings::load();
    let theme = load_theme(&config.theme);

    let size = crossterm::terminal::size().unwrap_or((120, 32));
    let startup_path = path;
    let mut app = App::new(config, theme, startup_path.clone(), size.0 as usize, size.1 as usize);
    if let Some(path) = startup_path {
        app.open_startup(&path);
    }
    app.set_terminal_size(size.0 as usize, size.1 as usize);

    setup_terminal(mouse)?;
    TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Termination signals set the quit flag through the same central path,
    // so raw mode, alternate screen and PTYs are always released cleanly.
    let (signal_tx, mut signal_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        wait_for_termination_signal().await;
        let _ = signal_tx.send(());
    });

    let result = run(&mut terminal, &mut app, &mut signal_rx).await;

    app.shutdown();
    restore_terminal(mouse)?;
    TERMINAL_ACTIVE.store(false, Ordering::SeqCst);
    terminal.show_cursor()?;

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("AMO VS exited with an error: {:#}", e);
            Err(e)
        }
    }
}

async fn wait_for_termination_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut int = signal(SignalKind::interrupt()).ok();
        let mut term = signal(SignalKind::terminate()).ok();
        let mut hup = signal(SignalKind::hangup()).ok();
        tokio::select! {
            _ = async { if let Some(s) = int.as_mut() { s.recv().await; } else { futures::future::pending::<()>().await; } } => {}
            _ = async { if let Some(s) = term.as_mut() { s.recv().await; } else { futures::future::pending::<()>().await; } } => {}
            _ = async { if let Some(s) = hup.as_mut() { s.recv().await; } else { futures::future::pending::<()>().await; } } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    signal_rx: &mut tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(60));
    let mut needs_draw = true;

    loop {
        tokio::select! {
            _ = &mut *signal_rx => {
                app.quit = true;
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if is_mouse_event(&event) && !mouse_enabled() { continue; }
                        handle_terminal_event(app, event);
                        needs_draw = true;
                    }
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            _ = ticker.tick() => {
                app.sync_terminal_size();
                let terminal_changed = app.terminal.pump_all();
                app.check_files();
                app.refresh_timers(std::time::Instant::now());
                if terminal_changed { needs_draw = true; }
            }
        }

        if app.quit { break; }
        if needs_draw {
            terminal.draw(|frame| ui::render(frame, app))?;
            needs_draw = false;
        }
    }
    Ok(())
}

fn is_mouse_event(event: &Event) -> bool {
    matches!(event, Event::Mouse(_))
}

fn mouse_enabled() -> bool {
    MOUSE_ENABLED.load(Ordering::Relaxed)
}

fn handle_terminal_event(app: &mut App, event: Event) {
    amo_vs::app::events::handle(app, event);
}

fn setup_terminal(mouse: bool) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    if mouse { execute!(stdout, EnableMouseCapture)?; }
    if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }
    Ok(())
}

fn restore_terminal(mouse: bool) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    // Single atomic write so the restore is not interleaved with other output.
    stdout.write_all(&terminal_restore_sequence(mouse))?;
    stdout.flush()?;
    // Ignore crossterm's duplicate structured writes; the sequence above is
    // equivalent and works even when stdout is a PTY being torn down.
    let _ = (DisableMouseCapture, LeaveAlternateScreen, PopKeyboardEnhancementFlags, cursor::Show);
    disable_raw_mode()?;
    Ok(())
}
