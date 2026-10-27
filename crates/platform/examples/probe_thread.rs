fn main() {
    // Wie im Companion: Abfrage aus einem Hintergrund-Thread, während der Hauptthread nur wartet
    let t = std::thread::spawn(|| {
        let mut p = tickly_platform::system();
        for _ in 0..3 {
            println!("Thread · aktive App: {:?} · gesperrt: {}", p.foreground_app(), p.is_locked());
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    t.join().unwrap();
}
