use dplayer::cli::{self, Launch};

fn main() -> Result<(), String> {
    match cli::parse(std::env::args_os().skip(1))? {
        Launch::Play(paths) => dplayer::run(paths),
        Launch::Settings => dplayer::app::show_settings(),
        Launch::Help => println!("{}", cli::HELP),
        Launch::Version => println!("Dplayer {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
