fn main() {
    let exit_code = meiksh::run_from_env();
    std::process::exit(exit_code);
}
