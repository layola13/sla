use super::*;
use std::path::PathBuf;

struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> TempTree {
        let dir = std::env::temp_dir().join(format!(
            "sla_workspace_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempTree(dir)
    }
    fn write(&self, rel: &str, content: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    fn root(&self) -> String {
        normalize_slashes(
            &std::fs::canonicalize(&self.0)
                .unwrap()
                .to_string_lossy(),
        )
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn two_member_tree(tag: &str) -> TempTree {
    let tree = TempTree::new(tag);
    tree.write(
        "sa.mod",
        "workspace {\n  members [\"members/app\", \"members/tool\"]\n  default_member \"app\"\n}\n",
    );
    tree.write("members/app/sa.mod", "package \"app\"\n");
    tree.write("members/app/src/main.sla", "fn main() -> i32 { return 0; }\n");
    tree.write("members/tool/sa.mod", "package \"tool\"\n");
    tree.write("members/tool/src/main.sla", "fn main() -> i32 { return 1; }\n");
    tree
}

// The three Zig unit tests of workspace.zig, ported.

#[test]
fn selects_default_member_by_package_name() {
    let tree = two_member_tree("default");
    let resolved = resolve_from_root_path(&tree.root(), None).unwrap();
    assert_eq!(
        resolved.workspace_rel_member_path.as_deref(),
        Some("members/app")
    );
    assert_eq!(resolved.selected_package.as_deref(), Some("app"));
    let source_path = selected_source_path(&resolved).unwrap();
    assert!(source_path.ends_with("members/app/src/main.sla"));
}

#[test]
fn selects_explicit_member_by_relative_path() {
    let tree = TempTree::new("explicit");
    tree.write(
        "sa.mod",
        "workspace {\n  members [\"members/tool\"]\n  default_member \"members/tool\"\n}\n",
    );
    tree.write("members/tool/sa.mod", "package \"tool\"\n");
    tree.write("members/tool/src/main.sla", "fn main() -> i32 { return 1; }\n");
    let resolved = resolve_from_root_path(&tree.root(), Some("members/tool")).unwrap();
    assert_eq!(
        resolved.workspace_rel_member_path.as_deref(),
        Some("members/tool")
    );
    assert_eq!(resolved.selected_package.as_deref(), Some("tool"));
}

#[test]
fn climbs_from_member_manifest_to_ancestor_workspace_root() {
    let tree = two_member_tree("climb");
    // Zig's third test uses default_member "tool" with the app
    // member as the climb source; rebuild that exact shape.
    tree.write(
        "sa.mod",
        "workspace {\n  members [\"members/app\", \"members/tool\"]\n  default_member \"tool\"\n}\n",
    );
    let start = tree.0.join("members/app/src");
    let resolved =
        resolve_from_root_path(&normalize_slashes(&start.to_string_lossy()), None).unwrap();
    assert_eq!(resolved.workspace_root, tree.root());
    assert!(resolved.member_root.ends_with("members/app"));
    assert_eq!(
        resolved.workspace_rel_member_path.as_deref(),
        Some("members/app")
    );
    assert_eq!(resolved.selected_package.as_deref(), Some("app"));
}

#[test]
fn manifest_parse_quirks() {
    // Unknown top-level lines are ignored; unknown workspace
    // keys are InvalidFormat; duplicates are DuplicateEntry.
    let m = parse_manifest("nonsense line\npackage \"x\"\n").unwrap();
    assert_eq!(m.package_name.as_deref(), Some("x"));
    assert!(m.workspace.is_none());
    assert_eq!(
        parse_manifest("workspace {\n  bogus 1\n}\n"),
        Err(InvalidFormat)
    );
    assert_eq!(
        parse_manifest("package \"a\"\npackage \"b\"\n"),
        Err(DuplicateEntry)
    );
    assert_eq!(
        parse_manifest("workspace {\n  members [\"a\", \"a\"]\n}\n"),
        Err(DuplicateEntry)
    );
    // Unterminated workspace block.
    assert_eq!(
        parse_manifest("workspace {\n  members []\n"),
        Err(InvalidFormat)
    );
    // Comment stripping: `//` after whitespace only.
    let m = parse_manifest("package \"a\" // trailing\n").unwrap();
    assert_eq!(m.package_name.as_deref(), Some("a"));
}

#[test]
fn unknown_package_and_missing_default_are_loud() {
    let tree = two_member_tree("loud");
    assert_eq!(
        resolve_from_root_path(&tree.root(), Some("nope")),
        Err(UnknownPackage)
    );
    let tree2 = TempTree::new("nodefault");
    tree2.write(
        "sa.mod",
        "workspace {\n  members [\"members/app\", \"members/tool\"]\n}\n",
    );
    tree2.write("members/app/sa.mod", "package \"app\"\n");
    tree2.write("members/tool/sa.mod", "package \"tool\"\n");
    assert_eq!(
        resolve_from_root_path(&tree2.root(), None),
        Err(MissingDefaultMember)
    );
}
