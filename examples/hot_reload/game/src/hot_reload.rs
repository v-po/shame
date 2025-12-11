use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

pub struct HotReload {
    tx_request: SyncSender<()>,
    rx_result: Receiver<Option<String>>,
}

impl HotReload {
    pub fn new(server_addr: String) -> Self {
        let busy = Arc::new(AtomicBool::new(false));
        let (tx_request, rx_request) = mpsc::sync_channel::<()>(1);
        let (tx_result, rx_result) = mpsc::channel::<Option<String>>();

        let busy_clone = busy.clone();
        // worker thread
        thread::spawn(move || {
            for _ in rx_request {
                busy_clone.store(true, Ordering::Release);

                let start = Instant::now();
                let result = send_rebuild_request(&server_addr);
                // println!("{:?}", result);
                tx_result.send(result).ok();

                println!(
                    "[hot_reload] pipeline rebuilt in {}ms",
                    Instant::now().duration_since(start).as_millis()
                );

                busy_clone.store(false, Ordering::Release);
            }
        });

        Self { tx_request, rx_result }
    }

    pub fn request_rebuild(&self) {
        match self.tx_request.try_send(()) {
            Ok(_) => {
                println!("[hot_reload] rebuild requested");
            }
            Err(TrySendError::Full(_)) => {
                println!("[hot_reload] rebuild already in progress… ignoring");
            }
            Err(e) => {
                eprintln!("[hot_reload] send error: {}", e);
            }
        }
    }

    // pub fn request_rebuild(&self) {
    //     if self
    //         .busy
    //         .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
    //         .is_ok()
    //     {
    //         if self.tx_request.send(()).is_err() {
    //             eprintln!("[hot_reload] rebuild request dropped (channel closed)");
    //             self.busy.store(false, Ordering::Release);
    //         }
    //     } else {
    //         println!("[hot_reload] rebuild already in progress... ignoring");
    //     }
    // }

    pub fn try_get_new_pipeline_json(&self) -> Option<String> {
        self.rx_result.try_recv().ok().flatten()
    }
}

fn send_rebuild_request(server_addr: &str) -> Option<String> {
    fn log_err<E: std::fmt::Display>(step: &str, err: E) {
        eprintln!("[hot_reload] {}: {}", step, err);
    }

    println!("[hot_reload] sending rebuild request...");

    let mut stream = TcpStream::connect(server_addr)
        .map_err(|e| {
            log_err("connect failed", e);
        })
        .ok()?;
    let mut reader = BufReader::new(stream.try_clone().ok()?);

    // send `rebuild` command
    stream.write_all(b"rebuild\n").ok()?;
    stream.flush().ok()?;

    let mut len_line = String::new();
    reader.read_line(&mut len_line).ok()?;
    let len: usize = len_line.trim().parse().ok()?;

    let mut json = vec![0u8; len];
    reader.read_exact(&mut json).ok()?;

    let response = String::from_utf8(json).ok()?;

    if response.starts_with("ERR:") {
        eprintln!("[hot_reload] server error: {}", response);
        None
    } else {
        Some(response)
    }
}
