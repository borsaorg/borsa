use assert_cmd::prelude::*;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

fn is_numbered_rs(entry: &fs::DirEntry) -> bool {
    let path = entry.path();
    path.is_file()
        && path.extension() == Some(OsStr::new("rs"))
        && path
            .file_name()
            .and_then(OsStr::to_str)
            .map_or(false, |n| n.chars().next().map_or(false, |c| c.is_ascii_digit()))
}

#[test]
fn run_all_examples_with_mock() {
    let examples_dir = Path::new("examples");
    let entries = fs::read_dir(examples_dir).expect("read examples dir");
    let mut found_any = false;
    for entry in entries.flatten().filter(is_numbered_rs) {
        let path = entry.path();
        let name_owned = path
            .file_stem()
            .and_then(OsStr::to_str)
            .expect("example name")
            .to_string();
        found_any = true;
        let mut cmd = Command::new("cargo");
        cmd.arg("run").arg("--example").arg(&name_owned);
        cmd.env("BORSA_EXAMPLES_USE_MOCK", "1");
        cmd.assert().success();
    }
    assert!(found_any, "no examples found to run");
}
