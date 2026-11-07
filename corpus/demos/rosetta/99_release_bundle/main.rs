struct BundleManifest {
    binary: bool,
    config: bool,
    checksums: bool,
    signatures: bool,
}

fn release_ready(manifest: BundleManifest) -> bool {
    manifest.binary && manifest.config && manifest.checksums && manifest.signatures
}

fn main() {
    let manifest = BundleManifest { binary: true, config: true, checksums: true, signatures: true };
    println!("{}", release_ready(manifest));
}
