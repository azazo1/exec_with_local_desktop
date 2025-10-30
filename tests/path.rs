#[cfg(unix)]
#[test]
fn find_executable() {
    use std::path::PathBuf;

    assert_eq!(Ok(PathBuf::from("/bin/bash")), which::which("bash"));
}
