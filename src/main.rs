fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(meiksh::run_from_env() as u8)
}
