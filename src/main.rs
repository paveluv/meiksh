fn main() {
    let exit_code = meiksh::run_from_env();
    meiksh::sys::exit_process(exit_code);
}
