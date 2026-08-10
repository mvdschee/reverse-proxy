use thiserror::Error as DisplayError;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(DisplayError, Debug)]
pub enum Error {
	#[error("environment: `{0}` is not set")]
	Env(String),

	#[error("main loop closed")]
	MainLoopClosed,

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("file system error: {0}")]
	FileSystem(String),

	#[error("certificate error: {0}")]
	Certificate(String),

	#[error("Acme error: {0}")]
	Acme(String),

	#[error("proxy error: {0}")]
	Proxy(String),

	#[error("dns error: {0}")]
	Dns(String),

	#[error("config error: {0}")]
	Config(String),

	#[error("unknown error")]
	Unknown,
}
