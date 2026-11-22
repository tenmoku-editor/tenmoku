//! The tests' helper processes. Never shipped.
//!
//! - `fe-remote-client-fixture askpass <socket> <prompt>`: the askpass
//!   helper, as `tenmoku --askpass` is.
//! - `fe-remote-client-fixture server --build BUILD [--answer-any]
//!   [--fail-with MESSAGE] [--stdio | --version]`: a fake `tenmoku-server
//!   --stdio` serving one session on its stdio as `BUILD`. It refuses a
//!   `Hello` of another build, as the real one does, unless `--answer-any`,
//!   when it takes any `Hello` and answers as `BUILD`. `--fail-with` prints
//!   `MESSAGE` on stderr and exits, as a server that can't start does, after
//!   a thousand lines of noise: a log taken without waiting for stderr to
//!   end misses it.
//!   `--version` prints `tenmoku-server BUILD`, as the real one does.

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    use fe_remote_proto::testing::{RawPeer, accept_any};
    use fe_remote_proto::window::INITIAL_WINDOW;
    use fe_remote_proto::{Config, Link, Mux};
    use std::io::Write as _;
    use std::process::ExitCode;

    let mut args = std::env::args_os().skip(1);
    match args.next().and_then(|a| a.into_string().ok()).as_deref() {
        Some("askpass") => {
            let code = fe_git::askpass::helper_main(args);
            ExitCode::from(u8::try_from(code).unwrap_or(1))
        }
        Some("server") => {
            let mut build = None;
            let mut answer_any = false;
            let mut fail_with = None;
            let mut version = false;
            let mut args = args.filter_map(|a| a.into_string().ok());
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--build" => build = args.next(),
                    "--answer-any" => answer_any = true,
                    "--fail-with" => fail_with = args.next(),
                    "--version" => version = true,
                    "--stdio" => {}
                    _ => return ExitCode::from(2),
                }
            }
            let Some(build) = build else {
                return ExitCode::from(2);
            };
            if version {
                println!("tenmoku-server {build}");
                return ExitCode::SUCCESS;
            }
            if let Some(message) = fail_with {
                let mut stderr = std::io::stderr().lock();
                for n in 0..1000 {
                    let _ = writeln!(stderr, "starting, step {n} of a slow start");
                }
                let _ = writeln!(stderr, "{message}");
                return ExitCode::FAILURE;
            }
            let link = Link::new(std::io::stdin(), std::io::stdout());
            if answer_any {
                let mut peer = RawPeer::new(link);
                peer.welcome(&build, INITIAL_WINDOW);
                // The client closes the link on a build it isn't.
                while peer.recv().is_some() {}
                return ExitCode::SUCCESS;
            }
            let config = Config {
                build,
                heartbeat: None,
                ..Config::default()
            };
            if let Ok((mux, _)) = Mux::server(link, config, accept_any) {
                mux.wait_lost();
            }
            ExitCode::SUCCESS
        }
        _ => ExitCode::from(2),
    }
}

#[cfg(not(unix))]
fn main() {}
