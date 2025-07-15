use color_eyre::Result;
use crossterm::ExecutableCommand;

use synthrs::{App, run};

fn main() -> Result<()> {
    color_eyre::install()?;
    std::io::stdout()
        .execute(crossterm::event::EnableMouseCapture)
        .unwrap();

    let terminal = ratatui::init();
    let mut app = App::default();
    let result = run(&mut app, terminal);

    ratatui::restore();
    result
}
