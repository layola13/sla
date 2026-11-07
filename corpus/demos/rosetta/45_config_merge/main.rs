#[derive(Clone, Copy)]
struct Config {
    threads: u32,
    level: u32,
}

fn merge(base: Config, override_cfg: Config) -> Config {
    Config {
        threads: if override_cfg.threads == 0 { base.threads } else { override_cfg.threads },
        level: if override_cfg.level == 0 { base.level } else { override_cfg.level },
    }
}

fn main() {
    let merged = merge(
        Config { threads: 4, level: 1 },
        Config { threads: 0, level: 3 },
    );
    println!("{} {}", merged.threads, merged.level);
}
