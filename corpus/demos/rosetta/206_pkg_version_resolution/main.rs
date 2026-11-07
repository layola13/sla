fn manifest_version(text: &str) -> Option<(u32, u32, u32)> {
    let version_line = text.lines().find(|line| line.trim_start().starts_with("version = "))?;
    let version = version_line.split('"').nth(1)?;
    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

fn main() {
    let root = include_str!("sa.pkg");
    let old_pkg = include_str!("versions/v1_2_3/sa.pkg");
    let new_pkg = include_str!("versions/v1_4_0/sa.pkg");
    let resolver = include_str!("resolver/index.sa");

    let root_lists_both = root.contains("versions/v1_2_3") && root.contains("versions/v1_4_0");
    let old_version = manifest_version(old_pkg).expect("old version manifest");
    let new_version = manifest_version(new_pkg).expect("new version manifest");
    let resolver_imports_both = resolver.contains("v1_2_3/index.sa") && resolver.contains("v1_4_0/index.sa");
    let selected = if root_lists_both && resolver_imports_both && new_version > old_version {
        new_version.1 as i32
    } else {
        -1
    };

    println!("{}", selected);
}
