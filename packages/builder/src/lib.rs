pub mod detector;
pub mod git;
pub mod repository;
pub mod services;

#[allow(unused_imports)]
pub use detector::detect_project;
#[allow(unused_imports)]
pub use git::clone_or_pull;
#[allow(unused_imports)]
pub use services::{BuildResult, BuildRunner};
