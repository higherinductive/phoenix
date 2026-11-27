use std::process::ExitCode;

/// Usage: `phoenix <file>`.
/// Prints the typed AST of the expression, then its type.
fn main() -> ExitCode {
    let Some(src) = std::env::args().nth(1) else {
        eprintln!("No input file specified");
        return ExitCode::FAILURE;
    };

    let Ok(src) = std::fs::read_to_string(&src) else {
        eprintln!("Cannot read file `{src}`");
        return ExitCode::FAILURE;
    };

    let expr = match phoenix_parser::parse_expr(&src) {
        Ok(expr) => expr,
        Err(err) => {
            eprint!("{}", err.render(&src));
            return ExitCode::FAILURE;
        }
    };

    let typed = match phoenix_type::typecheck_expr(&expr) {
        Ok(typed) => typed,
        Err(err) => {
            eprintln!("Type error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let anf = phoenix_anf::to_anf(&typed);
    let closure_converted = phoenix_closure::closure_convert(anf);

    println!("{closure_converted}");
    ExitCode::SUCCESS
}
