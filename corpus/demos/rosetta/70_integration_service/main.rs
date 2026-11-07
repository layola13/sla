struct ServiceSnapshot {
    users_ok: bool,
    orders_ok: bool,
    users: i32,
    orders: i32,
}

fn integrated_total(snapshot: ServiceSnapshot) -> i32 {
    if !snapshot.users_ok {
        return -1;
    }
    if !snapshot.orders_ok {
        return -2;
    }
    snapshot.users + snapshot.orders
}

fn main() {
    let snapshot = ServiceSnapshot {
        users_ok: true,
        orders_ok: true,
        users: 2,
        orders: 4,
    };
    println!("{}", integrated_total(snapshot));
}
