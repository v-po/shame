use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Instant;

pub struct HotReload {
    busy: Arc<AtomicBool>,
    tx_request: Sender<()>,
    rx_result: Receiver<Option<String>>,
}

impl HotReload {
    pub fn new() -> Self {
        let busy = Arc::new(AtomicBool::new(false));
        let (tx_request, rx_request) = mpsc::channel::<()>();
        let (tx_result, rx_result) = mpsc::channel::<Option<String>>();

        let busy_clone = busy.clone();
        // worker thread
        thread::spawn(move || {
            for _ in rx_request {
                let start = Instant::now();
                let result = rebuild_pipeline();
                tx_result.send(result).ok();

                println!(
                    "[hot_reload] pipeline rebuilt in {}ms",
                    Instant::now().duration_since(start).as_millis()
                );

                busy_clone.store(false, Ordering::Release);
            }
        });

        Self {
            busy,
            tx_request,
            rx_result,
        }
    }

    pub fn request_rebuild(&self) {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            if self.tx_request.send(()).is_err() {
                eprintln!("[hot_reload] rebuild request dropped (channel closed)");
                self.busy.store(false, Ordering::Release);
            }
        } else {
            println!("[hot_reload] rebuild already in progress... ignoring");
        }
    }

    pub fn try_get_new_pipeline_json(&self) -> Option<String> {
        self.rx_result.try_recv().ok().flatten()
    }
}

fn rebuild_pipeline() -> Option<String> {
    println!("[hot_reload] building pipeline...");

    // build the pipeline-serialize binary (--offline avoids network stalls)
    let build = Command::new("cargo")
        .args(["build", "--offline", "-q", "-p", "pipeline", "--bin", "pipeline-serialize"])
        .status();

    match build {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("[hot_reload] cargo build failed with {status}");
            return None;
        }
        Err(e) => {
            eprintln!("[hot_reload] failed to run cargo build: {e}");
            return None;
        }
    }

    // run the binary and capture stdout (the serialized pipeline JSON)
    let run = Command::new("cargo")
        .args(["run", "--offline", "-q", "-p", "pipeline", "--bin", "pipeline-serialize"])
        .output();

    match run {
        Ok(output) if output.status.success() => {
            let json = String::from_utf8(output.stdout).ok()?;
            Some(json)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[hot_reload] pipeline-serialize failed: {stderr}");
            None
        }
        Err(e) => {
            eprintln!("[hot_reload] failed to run pipeline-serialize: {e}");
            None
        }
    }
}

// we could also a TCP socket
// fn send_rebuild_request(server_addr: &str) -> Option<String> {
//     println!("[hot_reload] sending rebuild request...");

//     let mut stream = TcpStream::connect(server_addr).ok()?;
//     let mut reader = BufReader::new(stream.try_clone().ok()?);

//     // send `rebuild` command
//     stream.write_all(b"rebuild\n").ok()?;
//     stream.flush().ok()?;

//     let mut len_line = String::new();
//     reader.read_line(&mut len_line).ok()?;
//     let len: usize = len_line.trim().parse().ok()?;

//     let mut json = vec![0u8; len];
//     reader.read_exact(&mut json).ok()?;

//     let response = String::from_utf8(json).ok()?;

//     if response.starts_with("ERR:") {
//         eprintln!("[hot_reload] server error: {}", response);
//         None
//     } else {
//         Some(response)
//     }
// }
