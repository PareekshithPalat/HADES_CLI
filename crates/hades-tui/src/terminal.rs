use std::io::{self, stdout, Stdout};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::{
    cursor::Show,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tracing::{debug, info};

use crate::error::TuiError;

static TERMINAL_INITIALIZED: AtomicBool = AtomicBool::new(false);
static IN_ALTERNATE_SCREEN: AtomicBool = AtomicBool::new(false);

/// Sets up a panic hook that restores the terminal before letting the default panic handler run.
pub fn install_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));
}

/// Restores the terminal to cooked mode, disables mouse capture, and leaves alternate screen.
pub fn restore_terminal() -> io::Result<()> {
    if TERMINAL_INITIALIZED.swap(false, Ordering::SeqCst) {
        debug!("Restoring terminal state");
        disable_raw_mode()?;
        if IN_ALTERNATE_SCREEN.swap(false, Ordering::SeqCst) {
            execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show)?;
        } else {
            execute!(stdout(), Show)?;
        }
    }
    Ok(())
}

/// Initializes a full-screen Ratatui terminal in the alternate screen with mouse capture.
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, TuiError> {
    install_panic_hook();
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    TERMINAL_INITIALIZED.store(true, Ordering::SeqCst);
    IN_ALTERNATE_SCREEN.store(true, Ordering::SeqCst);

    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    info!("Terminal initialized in full-screen alternate screen mode with mouse capture");
    Ok(terminal)
}

/// Backward-compatibility alias for modal screens.
pub fn init_modal_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, TuiError> {
    init_terminal()
}

/// Backward-compatibility alias for leaving modal screens.
pub fn leave_modal_terminal() -> io::Result<()> {
    Ok(())
}
