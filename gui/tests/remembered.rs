use super::*;

#[test]
fn a_width_remembered_is_read_back_and_nothing_remembered_reads_as_none() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("state/kanbrain-app");
    assert_eq!(drawer_width(&folder), None, "nothing remembered yet");
    remember_drawer_width(&folder, 520.5).unwrap();
    assert_eq!(drawer_width(&folder), Some(520.5), "the folder is made");
    remember_drawer_width(&folder, 700.).unwrap();
    assert_eq!(drawer_width(&folder), Some(700.));
    let files: Vec<_> = std::fs::read_dir(&folder).unwrap().map(|f| f.unwrap().file_name()).collect();
    assert_eq!(files, ["drawer-width"], "no temporary file is left");
}

#[test]
fn a_width_unreadable_or_no_width_reads_as_none() {
    let tmp = tempfile::tempdir().unwrap();
    for (text, read) in [("520\n", Some(520.)), (" 480 ", Some(480.))] {
        std::fs::write(tmp.path().join("drawer-width"), text).unwrap();
        assert_eq!(drawer_width(tmp.path()), read, "{text:?}");
    }
    for text in ["", "wide", "NaN", "inf", "-inf", "-300", "0", "1e40"] {
        std::fs::write(tmp.path().join("drawer-width"), text).unwrap();
        assert_eq!(drawer_width(tmp.path()), None, "{text:?}");
    }
}

#[test]
fn tests_remember_nothing_unless_they_name_a_folder() {
    assert_eq!(folder(), None, "never the real one");
    let tmp = tempfile::tempdir().unwrap();
    remember_in(tmp.path());
    assert_eq!(folder().as_deref(), Some(tmp.path()));
}
