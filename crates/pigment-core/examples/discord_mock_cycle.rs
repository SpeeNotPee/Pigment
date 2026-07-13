//! Prove Discord socket discovery + the full presence cycle against a mock
//! Discord listening at the real $XDG_RUNTIME_DIR/discord-ipc-0.
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;
use pigment_core::discord::{Activity, Client};

fn read_frame(c: &mut std::os::unix::net::UnixStream) -> (u32, Vec<u8>) {
    let mut h = [0u8; 8]; c.read_exact(&mut h).unwrap();
    let op = u32::from_le_bytes(h[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
    let mut p = vec![0u8; len]; c.read_exact(&mut p).unwrap(); (op, p)
}

fn main() {
    let sock = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap()).join("discord-ipc-0");
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).unwrap();
    let server = thread::spawn(move || {
        let (mut c, _) = listener.accept().unwrap();
        let (op, _) = read_frame(&mut c); assert_eq!(op, 0, "expected handshake");
        let ready = br#"{"cmd":"DISPATCH","evt":"READY"}"#;
        c.write_all(&{let mut h=[0u8;8]; h[4..8].copy_from_slice(&(ready.len() as u32).to_le_bytes()); h[0..4].copy_from_slice(&1u32.to_le_bytes()); h}).unwrap();
        c.write_all(ready).unwrap();
        let (_, set) = read_frame(&mut c);
        let (_, clear) = read_frame(&mut c);
        (String::from_utf8_lossy(&set).to_string(), String::from_utf8_lossy(&clear).to_string())
    });

    // Use discovery (connect, not connect_to) — this is the untested path.
    let mut client = Client::connect("999999999999999999").expect("discovery + handshake");
    client.set_activity(&Activity::playing("The Strongest Battlegrounds", Some(1783361301))).unwrap();
    client.clear_activity().unwrap();

    let (set, clear) = server.join().unwrap();
    let _ = std::fs::remove_file(&sock);
    println!("SET_ACTIVITY frame: {set}");
    println!("CLEAR frame:        {clear}");
    assert!(set.contains("The Strongest Battlegrounds"), "name missing");
    assert!(set.contains("1783361301"), "start timestamp missing");
    assert!(clear.contains("\"activity\":null"), "clear should null the activity");
    println!("\nOK — discovery + connect + set + clear all verified against a mock Discord.");
}
