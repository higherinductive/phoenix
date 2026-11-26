use std::process::ExitCode;

/// Usage: `phoenix [file]` (reads standard input when no argument is given).
/// Prints the parsed expression, then its AST.
fn main() -> ExitCode {
    let src = match std::env::args().nth(1) {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(src) => src,
            Err(err) => {
                eprintln!("Cannot read file `{path}`: {err}");
                return ExitCode::FAILURE;
            }
        },
        None => {
            eprintln!("No input file specified");
            return ExitCode::FAILURE;
        }
    };

    match phoenix_parser::parse_expr(&src) {
        Ok(expr) => {
            println!("{expr}");
            println!("{expr:#?}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprint!("{}", err.render(&src));
            ExitCode::FAILURE
        }
    }
}
