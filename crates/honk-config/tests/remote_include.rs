use std::fs;

use honk_config::{Config, parser::remote};

#[test]
fn remote_include_uses_cached_rules_in_declaration_order() {
    let directory = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(directory.path()).unwrap();
    let entry = root.join("config.dae");
    let first = "https://example.com/first.dae?token=one";
    let second = "https://example.com/second.dae";
    fs::write(&entry, format!("include {{ '{first}' '{second}' }}\nrouting {{ dport(80) -> direct\n fallback: direct }}")).unwrap();
    let seed = remote::local_sources(&entry).unwrap();
    assert_eq!(remote::urls(&seed.sources).unwrap(), [first, second]);
    assert_eq!(seed.config.routing.rules.len(), 1);
    assert!(Config::from_file(entry.to_str().unwrap()).is_err());
    let first_path = remote::cache_path(&root, first).unwrap();
    fs::create_dir_all(first_path.parent().unwrap()).unwrap();
    fs::write(&first_path, "routing { dport(443) -> block }").unwrap();
    fs::write(
        remote::cache_path(&root, second).unwrap(),
        "routing { dport(53) -> direct }",
    )
    .unwrap();
    let config = Config::from_file(entry.to_str().unwrap()).unwrap();
    assert_eq!(
        config
            .routing
            .rules
            .iter()
            .map(|rule| rule.condition.port[0].as_str())
            .collect::<Vec<_>>(),
        ["80", "443", "53"]
    );
    assert!(!first_path.to_string_lossy().contains("token"));
    assert_ne!(
        first_path,
        remote::cache_path(&root, "https://example.com/first.dae?token=two").unwrap()
    );
}

#[test]
fn remote_include_rejects_duplicates_and_non_routing_content() {
    let directory = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(directory.path()).unwrap();
    let entry = root.join("config.dae");
    let url = "https://example.com/rules.dae";
    let path = remote::cache_path(&root, url).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "routing { fallback: direct }").unwrap();
    fs::write(&entry, format!("include {{ '{url}' '{url}' }}")).unwrap();
    assert!(Config::from_file(entry.to_str().unwrap()).is_err());
    fs::write(&entry, format!("include {{ '{url}' }}")).unwrap();
    for content in [
        "global { log_level: debug }",
        "routing { fallback: direct }\nsubscription { 'https://example.com/sub' }",
        "include { '../outside.dae' }",
        "routing { fallback: direct }\nunknown { hidden: value }",
    ] {
        fs::write(&path, content).unwrap();
        assert!(
            Config::from_file(entry.to_str().unwrap()).is_err(),
            "{content}"
        );
    }
}

#[cfg(unix)]
#[test]
fn remote_include_refuses_a_cache_symlink() {
    let directory = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(directory.path()).unwrap();
    let entry = root.join("config.dae");
    let external = root.join("local.dae");
    fs::write(&external, "routing { fallback: block }").unwrap();
    let url = "https://example.com/rules.dae";
    let path = remote::cache_path(&root, url).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(external, &path).unwrap();
    fs::write(&entry, format!("include {{ '{url}' }}")).unwrap();
    assert!(Config::from_file(entry.to_str().unwrap()).is_err());
}
