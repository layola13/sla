struct CliCommand<'a> {
    command: &'a str,
    release: bool,
}

fn exit_code(cmd: &CliCommand<'_>) -> i32 {
    if cmd.command == "build" && cmd.release { 0 } else { 2 }
}

fn main() {
    let cmd = CliCommand { command: "build", release: true };
    println!("{}", exit_code(&cmd));
}
