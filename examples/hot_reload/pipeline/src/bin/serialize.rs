fn main() {
    match pipeline::make_pipeline() {
        Ok(p) => print!("{}", serde_json::to_string(&p).unwrap()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
