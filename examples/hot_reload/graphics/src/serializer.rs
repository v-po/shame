use std::{fs, path::Path, process};

fn main() {
    let pipeline = match pipeline::make_pipeline() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("make_pipeline() failed: {:?}", e);
            process::exit(1);
        }
    };

    let json = match serde_json::to_string_pretty(&pipeline) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("serialize failed: {}", e);
            process::exit(1);
        }
    };

    println!("{}", json);

    // fs::write(Path::new("../game/pipeline.json"), &json).unwrap();
}
