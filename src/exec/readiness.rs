use crate::core::condition::Condition;
use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

pub fn wait_ready(
    condition: &Condition,
    timeout: Option<Duration>,
    exit_file: &Path,
    log_file: &Path,
    poll_interval: Duration,
) -> bool {
    if let Condition::Delay(s) = condition {
        let duration =
            crate::core::ready_when::ReadyWhen::parse_duration(s).unwrap_or(Duration::from_secs(0));

        std::thread::sleep(duration);

        return true;
    }

    let deadline = timeout.map(|t| Instant::now() + t);

    loop {
        let ready = check_once(condition, exit_file, log_file);

        if ready {
            return true;
        }

        if let Some(deadline) = deadline
            && Instant::now() >= deadline
        {
            return false;
        }

        std::thread::sleep(poll_interval);
    }
}

fn check_once(condition: &Condition, exit_file: &Path, log_file: &Path) -> bool {
    match condition {
        Condition::Delay(_) => unreachable!("tratado antes do loop de polling"),
        Condition::Port(port) => {
            let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, *port));
            TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
        }
        Condition::Http(url) => ureq::get(url)
            .call()
            .map(|r| r.status().is_success())
            .unwrap_or(false),
        Condition::File(path) => Path::new(path).exists(),
        Condition::Exit(expected) => fs::read_to_string(exit_file)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .map(|code| code == *expected)
            .unwrap_or(false),
        Condition::Log(pattern) => {
            let re = regex::Regex::new(pattern).expect("validado: regex compila");
            fs::read_to_string(log_file)
                .map(|content| re.is_match(&content))
                .unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tempfile::tempdir;

    #[test]
    fn delay_is_always_ready() {
        let ready = wait_ready(
            &Condition::Delay("0s".into()),
            None,
            Path::new("/nonexistent"),
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(ready);
    }

    #[test]
    fn port_ready_when_listening() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let ready = wait_ready(
            &Condition::Port(port),
            Some(Duration::from_millis(500)),
            Path::new("/nonexistent"),
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(ready);
        drop(listener);
    }

    #[test]
    fn port_times_out_when_closed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let ready = wait_ready(
            &Condition::Port(port),
            Some(Duration::from_millis(100)),
            Path::new("/nonexistent"),
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(!ready);
    }

    #[test]
    fn file_ready_when_it_exists() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("app.sock");
        fs::write(&file, "").unwrap();

        let ready = wait_ready(
            &Condition::File(file.to_string_lossy().to_string()),
            Some(Duration::from_millis(200)),
            Path::new("/nonexistent"),
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(ready);
    }

    #[test]
    fn file_times_out_when_missing() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("never.sock");

        let ready = wait_ready(
            &Condition::File(file.to_string_lossy().to_string()),
            Some(Duration::from_millis(100)),
            Path::new("/nonexistent"),
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(!ready);
    }

    #[test]
    fn exit_ready_when_code_matches() {
        let dir = tempdir().unwrap();
        let exit_file = dir.path().join("pane.exit");
        fs::write(&exit_file, "0\n").unwrap();

        let ready = wait_ready(
            &Condition::Exit(0),
            Some(Duration::from_millis(100)),
            &exit_file,
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(ready);
    }

    #[test]
    fn exit_times_out_when_code_differs() {
        let dir = tempdir().unwrap();
        let exit_file = dir.path().join("pane.exit");
        fs::write(&exit_file, "1\n").unwrap();

        let ready = wait_ready(
            &Condition::Exit(0),
            Some(Duration::from_millis(100)),
            &exit_file,
            Path::new("/nonexistent"),
            Duration::from_millis(10),
        );

        assert!(!ready);
    }

    #[test]
    fn log_ready_when_pattern_matches() {
        let dir = tempdir().unwrap();
        let log_file = dir.path().join("pane.log");
        fs::write(&log_file, "server listening on 3000\n").unwrap();

        let ready = wait_ready(
            &Condition::Log("listening on \\d+".into()),
            Some(Duration::from_millis(100)),
            Path::new("/nonexistent"),
            &log_file,
            Duration::from_millis(10),
        );

        assert!(ready);
    }

    #[test]
    fn log_times_out_when_pattern_absent() {
        let dir = tempdir().unwrap();
        let log_file = dir.path().join("pane.log");
        fs::write(&log_file, "still booting\n").unwrap();

        let ready = wait_ready(
            &Condition::Log("listening on \\d+".into()),
            Some(Duration::from_millis(100)),
            Path::new("/nonexistent"),
            &log_file,
            Duration::from_millis(10),
        );

        assert!(!ready);
    }
}
