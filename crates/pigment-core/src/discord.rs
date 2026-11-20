/// discord stuff
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

/// biggest frame i'll accept. real replies r a few KiB, a bogus len used to alloc up to 4 GiB
const MAX_FRAME: usize = 64 * 1024;

/// The Rich Presence payload | w stack overflow + opus
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Activity {
    pub details: Option<String>,
    pub state: Option<String>,
    pub start_unix: Option<i64>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
}

impl Activity {
    pub fn playing(game: &str, since_unix: Option<i64>) -> Self {
        Self {
            details: Some(game.to_string()),
            state: Some("Playing on Linux via Pigment".to_string()),
            start_unix: since_unix,
            large_image: Some("pigment".to_string()),
            large_text: Some("Pigment".to_string()),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        if let Some(d) = &self.details {
            obj.insert("details".into(), d.clone().into());
        }
        if let Some(s) = &self.state {
            obj.insert("state".into(), s.clone().into());
        }
        if let Some(t) = self.start_unix {
            obj.insert("timestamps".into(), serde_json::json!({ "start": t }));
        }
        if self.large_image.is_some() || self.large_text.is_some() {
            let mut assets = serde_json::Map::new();
            if let Some(i) = &self.large_image {
                assets.insert("large_image".into(), i.clone().into());
            }
            if let Some(t) = &self.large_text {
                assets.insert("large_text".into(), t.clone().into());
            }
            obj.insert("assets".into(), assets.into());
        }
        serde_json::Value::Object(obj)
    }
}

pub struct Client {
    stream: UnixStream,
    nonce: u64,
}

impl Client {
    /// try every candidate socket til one actually handshakes. a stale socket file from a
    /// crashed discord used to win just by existing n block the live one
    pub fn connect(client_id: &str) -> io::Result<Self> {
        let mut last_err = io::Error::new(io::ErrorKind::NotFound, "no discord-ipc socket found");
        for path in ipc_socket_candidates() {
            match Self::connect_to(&path, client_id) {
                Ok(c) => return Ok(c),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    pub fn connect_to(path: &std::path::Path, client_id: &str) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut client = Self { stream, nonce: 0 };

        let handshake = serde_json::json!({ "v": 1, "client_id": client_id });
        client.send(OP_HANDSHAKE, &handshake)?;
        let (opcode, _payload) = client.recv()?;
        if opcode == OP_CLOSE {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "discord rejected the handshake",
            ));
        }
        Ok(client)
    }

    pub fn set_activity(&mut self, activity: &Activity) -> io::Result<()> {
        self.set_activity_value(activity.to_json())
    }

    pub fn clear_activity(&mut self) -> io::Result<()> {
        self.set_activity_value(serde_json::Value::Null)
    }

    fn set_activity_value(&mut self, activity: serde_json::Value) -> io::Result<()> {
        self.nonce += 1;
        let frame = serde_json::json!({
            "cmd": "SET_ACTIVITY",
            "nonce": self.nonce.to_string(),
            "args": { "pid": std::process::id(), "activity": activity },
        });
        self.send(OP_FRAME, &frame)?;
        //read the reply so a dead/rejecting discord surfaces as an error n the caller reconnects
        let (opcode, payload) = self.recv()?;
        if opcode == OP_CLOSE {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "discord closed the connection",
            ));
        }
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&payload) {
            if v.get("evt").and_then(|e| e.as_str()) == Some("ERROR") {
                let msg = v["data"]["message"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string();
                return Err(io::Error::other(format!("discord: {msg}")));
            }
        }
        Ok(())
    }

    fn send(&mut self, opcode: u32, payload: &serde_json::Value) -> io::Result<()> {
        let bytes = serde_json::to_vec(payload)?;
        self.stream
            .write_all(&encode_header(opcode, bytes.len() as u32))?;
        self.stream.write_all(&bytes)?;
        self.stream.flush()
    }

    fn recv(&mut self) -> io::Result<(u32, Vec<u8>)> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header)?;
        let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        if len > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("discord sent a {len} byte frame, refusing"),
            ));
        }
        let mut payload = vec![0u8; len];
        self.stream.read_exact(&mut payload)?;
        Ok((opcode, payload))
    }
}

fn encode_header(opcode: u32, len: u32) -> [u8; 8] {
    let mut h = [0u8; 8];
    h[0..4].copy_from_slice(&opcode.to_le_bytes());
    h[4..8].copy_from_slice(&len.to_le_bytes());
    h
}

/// every place a discord-ipc socket might live, best guess first. had to manually figure
/// this one out. covers native, flatpak discord/canary/vesktop, snap, n the tmp fallbacks
/// for when XDG_RUNTIME_DIR is unset
fn ipc_socket_candidates() -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        bases.push(PathBuf::from(rt));
    }
    for var in ["TMPDIR", "TMP", "TEMP"] {
        if let Some(v) = std::env::var_os(var).filter(|v| !v.is_empty()) {
            bases.push(PathBuf::from(v));
        }
    }
    bases.push(PathBuf::from("/tmp"));

    let subdirs = [
        "",
        "app/com.discordapp.Discord",
        "app/com.discordapp.DiscordCanary",
        "app/dev.vencord.Vesktop",
        ".flatpak/com.discordapp.Discord/xdg-run",
        ".flatpak/com.discordapp.DiscordCanary/xdg-run",
        ".flatpak/dev.vencord.Vesktop/xdg-run",
        "snap.discord",
    ];
    let mut out = Vec::new();
    for base in &bases {
        for sub in subdirs {
            let dir = if sub.is_empty() {
                base.clone()
            } else {
                base.join(sub)
            };
            for i in 0..10 {
                let candidate = dir.join(format!("discord-ipc-{i}"));
                if candidate.exists() && !out.contains(&candidate) {
                    out.push(candidate);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn header_is_little_endian_opcode_then_length() {
        let h = encode_header(1, 258);
        assert_eq!(h, [1, 0, 0, 0, 2, 1, 0, 0]);
    }

    #[test]
    fn activity_json_includes_set_fields_only() {
        let a = Activity {
            details: Some("RIVALS".into()),
            state: Some("In a game".into()),
            start_unix: Some(1_700_000_000),
            large_image: Some("logo".into()),
            large_text: None,
        };
        let v = a.to_json();
        assert_eq!(v["details"], "RIVALS");
        assert_eq!(v["timestamps"]["start"], 1_700_000_000i64);
        assert_eq!(v["assets"]["large_image"], "logo");
        assert!(v.get("state").is_some());
    }

    /// A mock discord that accepts the handshake
    #[test]
    fn client_handshakes_and_sends_activity_over_a_socket() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&sock).unwrap();

        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let (op, payload) = read_frame(&mut conn);
            assert_eq!(op, OP_HANDSHAKE);
            let hs: serde_json::Value = serde_json::from_slice(&payload).unwrap();
            assert_eq!(hs["client_id"], "123456789");
            let ready =
                serde_json::to_vec(&serde_json::json!({ "cmd": "DISPATCH", "evt": "READY" }))
                    .unwrap();
            conn.write_all(&encode_header(OP_FRAME, ready.len() as u32))
                .unwrap();
            conn.write_all(&ready).unwrap();
            let (op2, payload2) = read_frame(&mut conn);
            assert_eq!(op2, OP_FRAME);
            let ack =
                serde_json::to_vec(&serde_json::json!({ "cmd": "SET_ACTIVITY", "evt": null }))
                    .unwrap();
            conn.write_all(&encode_header(OP_FRAME, ack.len() as u32))
                .unwrap();
            conn.write_all(&ack).unwrap();
            serde_json::from_slice::<serde_json::Value>(&payload2).unwrap()
        });

        let mut client = Client::connect_to(&sock, "123456789").unwrap();
        client
            .set_activity(&Activity {
                details: Some("The Strongest Battlegrounds".into()),
                state: Some("In a game".into()),
                start_unix: Some(42),
                ..Default::default()
            })
            .unwrap();

        let frame = server.join().unwrap();
        assert_eq!(frame["cmd"], "SET_ACTIVITY");
        assert_eq!(
            frame["args"]["activity"]["details"],
            "The Strongest Battlegrounds"
        );
        assert_eq!(frame["args"]["activity"]["timestamps"]["start"], 42);
        assert!(frame["args"]["pid"].as_u64().is_some());
    }

    #[test]
    fn huge_frame_len_is_refused_not_allocated() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&sock).unwrap();
        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let _ = read_frame(&mut conn);
            //claim a 4 GiB payload
            conn.write_all(&encode_header(OP_FRAME, u32::MAX)).unwrap();
        });
        let err = Client::connect_to(&sock, "1").err().expect("should refuse");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        server.join().unwrap();
    }

    #[test]
    fn dead_discord_surfaces_as_send_error() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&sock).unwrap();
        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let _ = read_frame(&mut conn);
            let ready = br#"{"evt":"READY"}"#;
            conn.write_all(&encode_header(OP_FRAME, ready.len() as u32))
                .unwrap();
            conn.write_all(ready).unwrap();
            //discord "restarts": hang up
        });
        let mut c = Client::connect_to(&sock, "1").unwrap();
        server.join().unwrap();
        assert!(c.set_activity(&Activity::playing("x", None)).is_err());
    }

    fn read_frame(conn: &mut UnixStream) -> (u32, Vec<u8>) {
        let mut header = [0u8; 8];
        conn.read_exact(&mut header).unwrap();
        let op = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let mut payload = vec![0u8; len];
        conn.read_exact(&mut payload).unwrap();
        (op, payload)
    }
}
