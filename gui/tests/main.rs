use super::*;
use anyhow::anyhow;

#[test]
fn the_command_line_says_its_errors_on_stderr_only() {
    let refused = start(vec!["help".into(), "nothing".into()], || panic!("the app opened"), |_| panic!("shown"));
    assert!(refused.unwrap_err().to_string().starts_with("unknown command `nothing`"));
}

#[test]
fn the_app_shows_why_it_couldnt_open() {
    let mut shown = None;
    let failed =
        start(Vec::new(), || Err(anyhow!("KANBRAIN_DIR must be an absolute path")), |e| shown = Some(e.to_string()));
    assert_eq!(shown.as_deref(), Some("KANBRAIN_DIR must be an absolute path"));
    assert!(failed.is_err(), "it still fails");
    start(Vec::new(), || Ok(()), |_| panic!("an app that opened shows no error")).unwrap();
}
