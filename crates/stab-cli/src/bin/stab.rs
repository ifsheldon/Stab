use std::process::ExitCode;

fn main() -> ExitCode {
    let code = stab_cli::run_from(
        std::env::args_os(),
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        std::io::stderr().lock(),
    );
    match u8::try_from(code) {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::FAILURE,
    }
}
