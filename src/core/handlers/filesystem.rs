use crate::{Error, Result, core::models::filesystem::SafePath};
use std::{
	fs,
	path::{Component, Path, PathBuf},
};

pub fn write_file(file_path: SafePath, content: &[u8]) -> Result<()> {
	// foxguard: ignore[rs/no-path-traversal]
	// validated with SafePath
	let path = Path::new(file_path.as_str());
	if let Some(parent) = path.parent()
		&& !parent.exists()
	{
		return Err(Error::FileSystem(format!("Parent directory {:?} does not exist", parent)));
	}

	fs::write(file_path.as_str(), content)
		.map_err(|e| Error::FileSystem(format!("Failed to write {}: {}", file_path, e)))?;

	Ok(())
}

pub fn read_file(file_path: &SafePath) -> Result<Vec<u8>> {
	let file = fs::read(file_path.as_str())
		.map_err(|e| Error::FileSystem(format!("Failed to read {}: {}", file_path, e)))?;

	Ok(file)
}

pub fn check_file_exists(file_path: &SafePath) -> bool {
	// foxguard: ignore[rs/no-path-traversal]
	// validated with SafePath
	Path::new(file_path.as_str()).exists()
}

/// Constructs a safe path by joining `base` and `file_path` and normalizing it.
/// It prevents directory traversal attacks by ensuring `file_path` does not escape `base` logically.
/// Does not require the file to exist.
pub fn safe_path(base: &str, file_path: &str) -> Result<SafePath> {
	// foxguard: ignore[rs/no-path-traversal]
	// no user input is used here, so we can safely use Path::new directly
	let base_path = Path::new(base);
	// foxguard: ignore[rs/no-path-traversal]
	// user inputs is validated before returning full_path
	let file_path = Path::new(file_path);

	// Normalize the user path to resolve '..' and '.' without accessing the filesystem
	// This is critical because fs::canonicalize requires the path to exist.
	let mut components = Vec::new();

	for component in file_path.components() {
		match component {
			Component::Prefix(_) => {
				return Err(Error::FileSystem("Prefix component not allowed".to_string()));
			},
			Component::RootDir => {
				// Treat absolute paths as relative to the base, or reject them.
				// Here we just ignore root dir to treat "/etc/passwd" as "etc/passwd" relative to base.
				// Alternatively, return error. Current convention in web often strips leading slash.
			},
			Component::CurDir => {}, // .
			Component::ParentDir => {
				// ..
				if components.pop().is_none() {
					return Err(Error::FileSystem(
						"Path traverses above the base directory".to_string(),
					));
				}
			},
			Component::Normal(c) => components.push(c),
		}
	}

	let relative_path: PathBuf = components.iter().collect();
	let full_path = base_path.join(relative_path);

	// Ensure the result is valid UTF-8
	full_path
		.to_str()
		.map(|s| SafePath::from(s.to_string()))
		.ok_or_else(|| Error::FileSystem("Invalid UTF-8 path".to_string()))
}
