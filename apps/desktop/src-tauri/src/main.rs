fn main() {
    if let Err(error) = ai_voice_desktop::run() {
        eprintln!("AI Voice Studio startup failed: {error}");
        std::process::exit(1);
    }
}
