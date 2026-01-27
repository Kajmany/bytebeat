#[derive(Debug, Clone, Copy)]
pub struct Song {
    pub author: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub code: &'static str,
}

// Build.rs will add the const array below.
/// Included songs are copyright their respective owners.
