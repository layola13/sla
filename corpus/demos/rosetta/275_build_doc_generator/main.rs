fn main() {
    let config = include_str!("build/docgen.toml");
    let spec = include_str!("docs/spec.md");
    let api_index = include_str!("docs/api/index.md");
    let generated = include_str!("generated/docs/index.sa");

    let config_sets_doc_paths = config.contains("source = \"docs\"")
        && config.contains("output = \"generated/docs\"");
    let docs_contain_sources = spec.contains("collects interface comments")
        && api_index.contains("sa_print_bytes")
        && api_index.contains("generated doc sections");
    let generated_mentions_docs = generated.contains("build/docgen.toml")
        && generated.contains("docs/api/index.md")
        && generated.contains("#def DOC_PAGE_COUNT = 2");
    let doc_contract = config_sets_doc_paths && docs_contain_sources && generated_mentions_docs;

    println!("{}", doc_contract as i32);
}
