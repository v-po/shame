use std::{
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::PathBuf,
    process::Command,
    time::Instant,
};

use libloading::{Library, Symbol};
use shame::results::RenderPipeline;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:5000")?;
    println!("[graphics] Listening on 127.0.0.1:5000");

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = handle_client(s) {
                    eprintln!("[graphics] client error: {}", e);
                }
            }
            Err(e) => eprintln!("[graphics] accept error: {}", e),
        }
    }

    Ok(())
}

fn handle_client(mut s: TcpStream) -> std::io::Result<()> {
    println!("[graphics] handle_client");

    let mut reader = BufReader::new(s.try_clone()?);
    let mut cmd_buf = String::new();
    reader.read_line(&mut cmd_buf)?;
    let cmd = cmd_buf.trim();

    match cmd {
        "rebuild" => {
            let start = Instant::now();

            let status = Command::new("cargo")
                .env("RUSTFLAGS", "-Awarnings")
                .args(&["build", "--offline", "-q", "-p", "pipeline"])
                .status()?;

            if !status.success() {
                s.write_all(b"ERR: build failed\n")?;
                return Ok(());
            }

            println!(
                "[graphics] pipeline rebuilt in {}ms",
                Instant::now().duration_since(start).as_millis()
            );

            // or use [`serializer`] and write to a file or to stdout
            // or, `game` could load `pipeline` dynamically
            // or, use the output of a custom build.rs... ?
            dynamic_load_pipeline(&mut s)?;
        }
        _ => {
            s.write_all(b"ERR: unknown command\n")?;
            s.flush()?;
            s.shutdown(Shutdown::Write)?;
        }
    }
    Ok(())
}

fn dynamic_load_pipeline(s: &mut TcpStream) -> std::io::Result<()> {
    let lib_path = get_lib_path();

    let new_lib_path = PathBuf::from("../target/debug/libpipeline");
    // trick to get around `dlopen()` caching
    // std::fs::copy()
    // std::fs::remove_file(&so_path)?;

    if new_lib_path.exists() {
        std::fs::remove_file(&new_lib_path)?;
    }

    std::fs::hard_link(&lib_path, &new_lib_path)?;

    unsafe {
        let lib = Library::new(&new_lib_path).unwrap();
        let func: Symbol<unsafe extern "C" fn() -> *mut std::ffi::c_void> = lib.get(b"make_pipeline_ptr").unwrap();
        let ptr = func();

        if ptr.is_null() {
            eprintln!("[graphics] pipeline build failed");
        } else {
            let pipeline = Box::from_raw(ptr as *mut RenderPipeline);
            let json = serde_json::to_string(&*pipeline)?;
            let len_line = format!("{}\n", json.len());
            s.write_all(len_line.as_bytes())?;
            s.write_all(json.as_bytes())?;
            s.flush()?;
        }
    }

    Ok(())
}

fn get_lib_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    return PathBuf::from("../target/debug/libpipeline.so");

    #[cfg(target_os = "macos")]
    return PathBuf::from("../target/debug/libpipeline.dylib");

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    compile_error!("Unsupported platform!");
    unreachable!();
}
