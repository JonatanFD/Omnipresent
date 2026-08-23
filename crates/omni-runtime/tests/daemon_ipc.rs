//! Daemon IPC integration test: hello protocol version handshake and event subscriptions.

use omni_runtime::Paths;
use omni_runtime::ipc::{Event, PROTOCOL_VERSION, Request, Response};
use omni_runtime::ipc_transport::connect_blocking;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
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

fn wait_until_up(paths: &Paths, who: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(Response::Status(_)) = send(paths, &Request::Status) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("daemon {who} never came up");
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omni-it-{name}-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_port(paths: &Paths, port: u16) {
    let config = format!(r#"{{"port":{port}}}"#);
    std::fs::write(paths.config_file(), config).unwrap();
}

#[test]
fn hello_reports_the_protocol_version_and_subscribe_streams_changes() {
    let port = 49000 + (std::process::id() % 500) as u16;
    let dir = temp_dir("sub");
    let paths = Paths::at(dir.clone());
    write_port(&paths, port);

    let run = paths.clone();
    let handle = std::thread::spawn(move || {
        let _ = omni_runtime::run_with_paths(run);
    });
    wait_until_up(&paths, "sub");

    match send(&paths, &Request::Hello).expect("hello") {
        Response::Hello {
            protocol_version, ..
        } => assert_eq!(protocol_version, PROTOCOL_VERSION),
        other => panic!("unexpected hello reply: {other:?}"),
    }

    let (tx, rx) = mpsc::channel::<Event>();
    let sub_paths = paths.clone();
    let sub = std::thread::spawn(move || {
        let mut stream = connect_blocking(&sub_paths).expect("subscribe connect");
        let mut line = serde_json::to_string(&Request::Subscribe).unwrap();
        line.push('\n');
        stream.write_all(line.as_bytes()).expect("send subscribe");
        let mut reader = BufReader::new(stream);
        loop {
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Ok(event) = serde_json::from_str::<Event>(buf.trim_end())
                        && tx.send(event).is_err()
                    {
                        break;
                    }
                }
            }
        }
    });

    let Event::Status(initial) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("initial event");
    assert!(!initial.clipboard_sharing);

    assert!(matches!(
        send(&paths, &Request::Clipboard { enabled: true }).expect("toggle"),
        Response::Ok
    ));
    let deadline = Instant::now() + Duration::from_secs(10);
    let saw_change = loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Event::Status(s)) if s.clipboard_sharing => break true,
            Ok(_) => {}
            Err(_) if Instant::now() >= deadline => break false,
            Err(_) => {}
        }
    };
    assert!(
        saw_change,
        "subscriber was never pushed the clipboard change"
    );

    let _ = send(&paths, &Request::Stop);
    let _ = sub.join();
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&dir);
}
