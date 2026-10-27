//! Zeigt, was der Companion auf diesem Rechner erkennt: `cargo run -p tickly-platform --example probe`
fn main() {
    let mut p = tickly_platform::system();
    for _ in 0..3 {
        println!("{} · aktive App: {:?} · gesperrt: {}", p.os(), p.foreground_app(), p.is_locked());
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
