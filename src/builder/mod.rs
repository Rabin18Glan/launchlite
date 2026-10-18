pub mod detector;
pub mod git;
pub mod runner;

#[allow(unused_imports)]
pub use detector::detect_project;
#[allow(unused_imports)]
pub use git::clone_or_pull;
#[allow(unused_imports)]
pub use runner::{BuildResult, BuildRunner};
