#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        println!("\x1b[90m{} \x1b[32m{} \x1b[0m{}", chrono::Local::now().format("%H:%M:%S%.3f %d-%m-%y"), "[INFO]", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        println!("\x1b[90m{} \x1b[33m{} \x1b[0m{}", chrono::Local::now().format("%H:%M:%S%.3f %d-%m-%y"), "[WARN]", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        println!("\x1b[90m{} \x1b[31m{} \x1b[0m{}", chrono::Local::now().format("%H:%M:%S%.3f %d-%m-%y"), "[ERROR]", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! newtype {
    ($(#[$meta:meta])* $name:ident($inner:ty) $(, derive($($extra:path),+ $(,)?))?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq $(, $($extra),+)?)]
        pub struct $name($inner);

        impl ::std::ops::Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &$inner {
                &self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}
