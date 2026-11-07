fn main() {
    let api = include_str!("api/contract.sai");
    let layout = include_str!("layout/contract.sal");
    let implementation = include_str!("impl/contract.sa");

    let iface_declared = api.contains("@extern contract_score_contract") && api.contains("ptr");
    let layout_declared = layout.contains("Contract_SIZE = 4") && layout.contains("Contract_value = +0");
    let impl_exports_score = implementation.contains("@export contract_score") && implementation.contains("Contract_value");
    let separated_layers = iface_declared && layout_declared && impl_exports_score;

    println!("{}", separated_layers as i32);
}
