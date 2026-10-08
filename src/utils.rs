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
macro_rules! string_newtype {
    ($name:ident $(, derive($($extra:path),+ $(,)?))?) => {
        #[derive(Debug, Clone $(, $($extra),+)?)]
        pub struct $name(String);

        impl ::std::ops::Deref for $name {
            type Target = String;
            fn deref(&self) -> &String {
                &self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                $name(s)
            }
        }
    };
}
