use std::process::ExitCode;

fn main() -> ExitCode {
    let code = stab_cli::run_process();
    match u8::try_from(code) {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::FAILURE,
    }
}
