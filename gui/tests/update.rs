use super::*;

#[test]
fn a_program_replaced_since_it_started_restarts_from_the_new_file() {
    let replaced = running_file(PathBuf::from("/home/me/.local/bin/kanbrain (deleted)"));
    assert_eq!(replaced, PathBuf::from("/home/me/.local/bin/kanbrain"));
    assert_eq!(running_file(PathBuf::from("/opt/kanbrain")), PathBuf::from("/opt/kanbrain"));
}
