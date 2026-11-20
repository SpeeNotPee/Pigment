//! mock discord round trip. socket lives in a temp dir now, used to delete the REAL
//! $XDG_RUNTIME_DIR/discord-ipc-0 out from under a running discord lmao
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::thread;

use pigment_core::discord::{Activity, Client};

fn read_frame(c: &mut std::os::unix::net::UnixStream) -> (u32, Vec<u8>) {
    let mut h = [0u8; 8];
    c.read_exact(&mut h).unwrap();
    let op = u32::from_le_bytes(h[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
    let mut p = vec![0u8; len];
    c.read_exact(&mut p).unwrap();
    (op, p)
}

fn write_frame(c: &mut std::os::unix::net::UnixStream, body: &[u8]) {
    let mut h = [0u8; 8];
    h[0..4].copy_from_slice(&1u32.to_le_bytes());
    h[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
    c.write_all(&h).unwrap();
    c.write_all(body).unwrap();
}

fn main() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("discord-ipc-0");
    let listener = UnixListener::bind(&sock).unwrap();
    let server = thread::spawn(move || {
        let (mut c, _) = listener.accept().unwrap();
        let (op, _) = read_frame(&mut c);
        assert_eq!(op, 0, "expected handshake");
        write_frame(&mut c, br#"{"cmd":"DISPATCH","evt":"READY"}"#);
        let (_, set) = read_frame(&mut c);
        write_frame(&mut c, br#"{"cmd":"SET_ACTIVITY","evt":null}"#);
        let (_, clear) = read_frame(&mut c);
        write_frame(&mut c, br#"{"cmd":"SET_ACTIVITY","evt":null}"#);
        (
            String::from_utf8_lossy(&set).to_string(),
            String::from_utf8_lossy(&clear).to_string(),
        )
    });

    let mut client = Client::connect_to(&sock, "999999999999999999").expect("handshake");
    client
        .set_activity(&Activity::playing(
            "The Strongest Battlegrounds",
            Some(1783361301),
        ))
        .unwrap();
    client.clear_activity().unwrap();

    let (set, clear) = server.join().unwrap();
    println!("SET_ACTIVITY frame: {set}");
    println!("CLEAR frame:        {clear}");
    assert!(set.contains("The Strongest Battlegrounds"), "name missing");
    assert!(set.contains("1783361301"), "start timestamp missing");
    assert!(
        clear.contains("\"activity\":null"),
        "clear should null the activity"
    );
    println!("\nOK - connect + set + clear all verified against a mock Discord.");
}
