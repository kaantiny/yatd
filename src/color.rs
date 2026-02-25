use std::io::IsTerminal;

pub struct Theme {
    pub red: &'static str,
    pub green: &'static str,
    pub yellow: &'static str,
    pub blue: &'static str,
    pub bold: &'static str,
    pub reset: &'static str,
}

const ON: Theme = Theme {
    red: "\x1b[31m",
    green: "\x1b[32m",
    yellow: "\x1b[33m",
    blue: "\x1b[34m",
    bold: "\x1b[1m",
    reset: "\x1b[0m",
};

const OFF: Theme = Theme {
    red: "",
    green: "",
    yellow: "",
    blue: "",
    bold: "",
    reset: "",
};

fn use_color(is_tty: bool) -> bool {
    std::env::var_os("NO_COLOR").is_none() && is_tty
}

/// Colour theme for stdout.
pub fn stdout_theme() -> &'static Theme {
    if use_color(std::io::stdout().is_terminal()) {
        &ON
    } else {
        &OFF
    }
}

/// Colour theme for stderr.
pub fn stderr_theme() -> &'static Theme {
    if use_color(std::io::stderr().is_terminal()) {
        &ON
    } else {
        &OFF
    }
}
