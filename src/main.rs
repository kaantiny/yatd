use std::process;

fn main() {
    if let Err(e) = yatd::run() {
        let c = yatd::color::stderr_theme();
        eprintln!("{}error:{} {e}", c.red, c.reset);
        for cause in e.chain().skip(1) {
            eprintln!("{}caused by:{} {cause}", c.red, c.reset);
        }
        process::exit(1);
    }
}
