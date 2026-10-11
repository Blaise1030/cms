use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::time::{Duration, Instant};
static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn main() {
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let mode = std::env::args().nth(2).unwrap();
    for round in 0..10 {
        let t0 = Instant::now();
        let hs: Vec<_> = (0..n)
            .map(|_| {
                let mode = mode.clone();
                std::thread::spawn(move || {
                    let g = L.lock().unwrap();
                    let pair = native_pty_system()
                        .openpty(PtySize {
                            rows: 24,
                            cols: 80,
                            pixel_width: 0,
                            pixel_height: 0,
                        })
                        .unwrap();
                    let mut cmd = CommandBuilder::new("/bin/zsh");
                    cmd.arg("-lc");
                    cmd.arg("exec '/Users/blaise/Developer/cms/tests/fixtures/fake_cli.sh'");
                    let mut child = pair.slave.spawn_command(cmd).unwrap();
                    let pid = child.process_id().unwrap() as i32;
                    let waiter = std::thread::spawn(move || {
                        child.wait().ok();
                    });
                    drop(pair.slave);
                    drop(g);
                    let mut r = pair.master.try_clone_reader().unwrap();
                    let w = if mode.contains('w') {
                        Some(pair.master.take_writer().unwrap())
                    } else {
                        None
                    };
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let mut buf = [0u8; 8192];
                        let mut acc = Vec::new();
                        while let Ok(n @ 1..) = r.read(&mut buf) {
                            acc.extend_from_slice(&buf[..n]);
                            if String::from_utf8_lossy(&acc).contains("READY") {
                                let _ = tx.send(());
                            }
                        }
                    });
                    let ok = rx.recv_timeout(Duration::from_secs(5)).is_ok();
                    unsafe {
                        libc::killpg(pid, libc::SIGTERM);
                    }
                    waiter.join().ok();
                    drop(w);
                    ok
                })
            })
            .collect();
        let ok = hs
            .into_iter()
            .filter(|_| true)
            .map(|h| h.join().unwrap())
            .filter(|x| *x)
            .count();
        println!("round {round}: {ok}/{n} {:?}", t0.elapsed());
    }
}
