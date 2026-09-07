//! Stopping the daemon must leave it startable again, in the same process.
//!
//! This is the invariant the desktop app's Stop and Start buttons rest on. The
//! daemon runs on a thread of the app, so a Start that follows a Stop has to
//! bind the same UDP port and the same IPC endpoint the previous one held —
//! and on Windows the named pipe is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`,
//! which fails outright while the old instance is alive.
//!
//! It cost a user a dead-ended app: `run` takes a few seconds to unwind, the
//! app refused to start a second daemon during that window, and nothing
//! retried once the window passed.

use omni_runtime::Paths;
use omni_runtime::ipc::{Request, Response};
use omni_runtime::ipc_transport::connect_blocking;
use std::io::{BufRead, BufReader, Write};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn send(paths: &Paths, req: &Request) -> std::io::Result<Response> {
    let mut stream = connect_blocking(paths)?;
    let mut line = serde_json::to_string(req).unwrap();
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    Ok(serde_json::from_str(reply.trim_end()).expect("a JSON response line"))
}

fn wait_until_up(paths: &Paths, which: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Ok(Response::Status(_)) = send(paths, &Request::Status) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("the {which} daemon never came up");
}

/// Starts a daemon on its own thread, the way the desktop app does.
fn spawn(paths: Paths) -> JoinHandle<()> {
    std::thread::spawn(move || {
        // A port already in use is the failure this test exists to catch, so it
        // must not be swallowed.
        omni_runtime::run_with_paths(paths).expect("the daemon started");
    })
}

/// Stops the daemon and waits for its thread to end, which is what the app's
/// Stop does — and what makes the Start that follows able to bind anything.
fn stop_and_join(paths: &Paths, handle: JoinHandle<()>) {
    send(paths, &Request::Stop).expect("the stop request was accepted");

    let deadline = Instant::now() + Duration::from_secs(20);
    while !handle.is_finished() {
        assert!(
            Instant::now() < deadline,
            "the daemon thread never finished after Stop"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    handle.join().expect("the daemon thread ended cleanly");
}

/// A state directory of its own, on a port of its own.
///
/// The port matters: these tests run in parallel with each other and alongside
/// whatever daemon the developer has running, and the failure they are looking
/// for — "address already in use" — is exactly what a shared port produces.
/// Sharing one would make them fail for a reason that is not the bug.
fn temp_paths_on_port(name: &str, port: u16) -> Paths {
    // Short on purpose: the IPC socket lives in here, and a Unix domain socket
    // path may not exceed roughly a hundred characters. The system temp
    // directory already uses most of that on macOS.
    let dir = std::env::temp_dir().join(format!("omni-r{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let paths = Paths::at(dir);
    paths
        .ensure()
        .expect("the temp state directory was created");
    std::fs::write(paths.config_file(), format!(r#"{{"port":{port}}}"#))
        .expect("the port was written");
    paths
}

#[test]
fn the_daemon_can_be_started_again_after_being_stopped() {
    let paths = temp_paths_on_port("c", 47331);

    let first = spawn(paths.clone());
    wait_until_up(&paths, "first");
    stop_and_join(&paths, first);

    // The whole point: everything the first one held — the UDP socket, the IPC
    // endpoint, the log file — has to be free for the second.
    let second = spawn(paths.clone());
    wait_until_up(&paths, "second");
    stop_and_join(&paths, second);

    let _ = std::fs::remove_dir_all(paths.dir());
}

#[test]
fn a_stopped_daemon_stops_answering() {
    // Start must not be fooled into thinking a daemon is still there, and the
    // window must not keep reporting a session that has gone.
    let paths = temp_paths_on_port("g", 47332);

    let daemon = spawn(paths.clone());
    wait_until_up(&paths, "only");
    stop_and_join(&paths, daemon);

    assert!(
        send(&paths, &Request::Status).is_err(),
        "a stopped daemon answered a status request"
    );

    let _ = std::fs::remove_dir_all(paths.dir());
}

#[test]
fn three_cycles_in_a_row_still_work() {
    // Once could be luck with timing. A user who is debugging their setup will
    // press Stop and Start several times in a row.
    let paths = temp_paths_on_port("t", 47333);

    for round in 1..=3 {
        let daemon = spawn(paths.clone());
        wait_until_up(&paths, &format!("round {round}"));
        stop_and_join(&paths, daemon);
    }

    let _ = std::fs::remove_dir_all(paths.dir());
}
