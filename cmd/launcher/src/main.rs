fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match cerbena_launcher::run_with_args(&args) {
        Ok(output) => {
            if !output.trim().is_empty() {
                println!("{output}");
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}
