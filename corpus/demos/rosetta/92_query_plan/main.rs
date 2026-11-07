struct QueryPlan {
    table_scan_rows: i32,
    index_rows: i32,
    filter_cost: i32,
}

fn chosen_cost(plan: QueryPlan) -> i32 {
    let table_cost = plan.table_scan_rows;
    let index_cost = plan.index_rows + plan.filter_cost;
    table_cost.min(index_cost)
}

fn main() {
    let plan = QueryPlan { table_scan_rows: 120, index_rows: 18, filter_cost: 7 };
    println!("{}", chosen_cost(plan));
}
