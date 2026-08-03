/// discord stuff
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

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
    pub fn connect(client_id: &str) -> io::Result<Self> {
        let path = find_ipc_socket().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no discord-ipc socket found")
        })?;
        Self::connect_to(&path, client_id)
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
        self.send(OP_FRAME, &frame)
    }

    fn send(&mut self, opcode: u32, payload: &serde_json::Value) -> io::Result<()> {
        let bytes = serde_json::to_vec(payload)?;
        self.stream.write_all(&encode_header(opcode, bytes.len() as u32))?;
        self.stream.write_all(&bytes)?;
        self.stream.flush()
    }

    fn recv(&mut self) -> io::Result<(u32, Vec<u8>)> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header)?;
        let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
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

/// Locate a Discord IPC socket | had to manually figure this one out cause opus was a dumb fuck
fn find_ipc_socket() -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let base = PathBuf::from(runtime);
    let subdirs = [
        "",
        "app/com.discordapp.Discord",
        "app/com.discordapp.DiscordCanary",
        "snap.discord",
    ];
    for sub in subdirs {
        let dir = if sub.is_empty() { base.clone() } else { base.join(sub) };
        for i in 0..10 {
            let candidate = dir.join(format!("discord-ipc-{i}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
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
            let ready = serde_json::to_vec(&serde_json::json!({ "cmd": "DISPATCH", "evt": "READY" })).unwrap();
            conn.write_all(&encode_header(OP_FRAME, ready.len() as u32)).unwrap();
            conn.write_all(&ready).unwrap();
            let (op2, payload2) = read_frame(&mut conn);
            assert_eq!(op2, OP_FRAME);
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
        assert_eq!(frame["args"]["activity"]["details"], "The Strongest Battlegrounds");
        assert_eq!(frame["args"]["activity"]["timestamps"]["start"], 42);
        assert!(frame["args"]["pid"].as_u64().is_some());
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
