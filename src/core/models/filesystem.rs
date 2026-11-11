use crate::newtype;

// --- SAFEPATH ---
newtype!(
   SafePath(String)
);

impl From<String> for SafePath {
	fn from(s: String) -> Self {
		SafePath(s)
	}
}
