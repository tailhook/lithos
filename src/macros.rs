#![macro_escape]

macro_rules! try_str {
    ($expr:expr) => {
        try!(($expr).map_err(|e| format!("{}: {}", stringify!($expr), e)))
    }
}

