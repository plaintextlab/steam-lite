use steamlite::vdf::{parse, Value};

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIX}{name}")).unwrap()
}

#[test]
fn parses_app_manifest() {
    let doc = parse(&fixture("appmanifest_105600.acf")).unwrap();
    let st = doc.get("AppState").unwrap();
    assert_eq!(st.get_str("appid"), Some("105600"));
    assert_eq!(st.get_str("name"), Some("Terraria"));
    assert_eq!(st.get_str("SizeOnDisk"), Some("831365954"));
    assert_eq!(st.path(&["InstalledDepots", "105601", "manifest"]).and_then(Value::as_str), Some("1234"));
}

#[test]
fn keys_are_case_insensitive() {
    let doc = parse(&fixture("appmanifest_105600.acf")).unwrap();
    assert_eq!(doc.path(&["appstate", "SIZEONDISK"]).and_then(Value::as_str), Some("831365954"));
}

#[test]
fn unicode_names_survive() {
    let doc = parse(&fixture("appmanifest_553850.acf")).unwrap();
    assert_eq!(doc.path(&["AppState", "name"]).and_then(Value::as_str), Some("HELLDIVERS™ 2"));
}

#[test]
fn escaped_backslashes() {
    let doc = parse(&fixture("appmanifest_228980.acf")).unwrap();
    let v = doc.path(&["AppState", "InstallScripts", "228983"]).and_then(Value::as_str);
    assert_eq!(v, Some(r"_CommonRedist\vcredist\2010\installscript.vdf"));
}

#[test]
fn tricky_syntax() {
    let doc = parse(&fixture("tricky.vdf")).unwrap();
    let r = doc.get("Root").unwrap();
    assert_eq!(r.get_str("Quoted"), Some("has \"escaped\" quotes \\ and\ttab"));
    assert_eq!(r.get_str("Unquoted"), Some("bare_value"));
    assert_eq!(r.get_str("Platform"), Some("linux"));
    assert_eq!(r.get_str("Path"), Some(r"C:\Games\New Folder"));
    assert_eq!(r.get_str("Multi"), Some("line one\nline two"));
    assert_eq!(r.get_str("Empty"), Some(""));
    assert_eq!(r.path(&["nested", "deep", "key"]).and_then(Value::as_str), Some("v"));
    // Duplicate keys are preserved in order; get() returns the first.
    assert_eq!(r.get_str("Dup"), Some("first"));
    assert_eq!(r.entries().iter().filter(|(k, _)| k == "Dup").count(), 2);
}

#[test]
fn errors_report_line() {
    let e = parse("\"a\"\n{\n\"b\" \"c\"\n").unwrap_err();
    assert_eq!(e.line, 4);
    assert!(parse("\"a\" \"unterminated").is_err());
    assert!(parse("}").is_err());
    assert!(parse("\"lonely\"").is_err());
}

#[test]
fn empty_document() {
    assert_eq!(parse("").unwrap(), Value::Obj(vec![]));
    assert_eq!(parse("// only a comment\n").unwrap(), Value::Obj(vec![]));
}
