//! A room: one game's world, hosted by the server in a thread of its own.
//! Browsers come and go and say things; every tick the room says what
//! each of them should be told. The server does the sockets: a browser
//! that cannot keep up is closed and the room is told, never waited on.
//! The server also keeps it: it saves the world every ten seconds, loads
//! the newest save into a fresh room on boot, and asks it to hold still
//! before a deploy stops the process.

pub use crate::who::Who;

/// Messages for browsers, by connection.
#[derive(Default)]
pub struct Outbox(pub Vec<(u32, Vec<u8>)>);

impl Outbox {
    pub fn send(&mut self, conn: u32, msg: Vec<u8>) {
        self.0.push((conn, msg));
    }
}

pub trait Room: Send {
    /// Its address: the page connects to `/ws/<id>`.
    fn id(&self) -> &'static str;
    /// Ticks a second.
    fn hz(&self) -> u32;
    /// A browser is here: after its platform Hello (which the server keeps),
    /// or as a guest (soul 0) on any other first message or a second of
    /// silence. A watcher only looks (the hub's live preview, say): it
    /// cannot play, and it is not one of the people here.
    fn open(&mut self, conn: u32, who: &Who, out: &mut Outbox);
    /// A connection said Hello again (a new name, say): its Who now.
    fn who(&mut self, conn: u32, who: &Who) {
        let _ = (conn, who);
    }
    /// A browser said something (untrusted bytes).
    fn message(&mut self, conn: u32, bytes: &[u8], out: &mut Outbox);
    /// A browser left, or was let go.
    fn close(&mut self, conn: u32);
    /// One tick of the world.
    fn tick(&mut self, out: &mut Outbox);
    /// People here now, watchers aside: what "online" counts.
    fn people(&self) -> usize;
    /// Everyone in the game now, bots too: what the hub's card says is
    /// playing. A game without bots need not say.
    fn playing(&self) -> usize {
        self.people()
    }
    /// The world as bytes for a snapshot; None if nothing is worth keeping.
    fn save(&self) -> Option<Vec<u8>> {
        None
    }
    /// The payload of the room's newest snapshot, on a fresh room, once.
    /// Err only for bytes that should have loaded; a room that can start
    /// over returns Ok.
    fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let _ = bytes;
        Ok(())
    }
    /// Its format version, written into each snapshot.
    fn schema(&self) -> u16 {
        0
    }
    /// The server is about to stop: hold everyone still. The server tells
    /// every page `Still` itself, after this.
    fn still(&mut self, out: &mut Outbox) {
        let _ = out;
    }
    /// Messages a browser may fall behind by before it is let go.
    fn backlog(&self) -> usize {
        60
    }
    /// Names its bots go by, which no person may take.
    fn reserved(&self) -> &'static [&'static str] {
        &[]
    }
    /// Extra numbers for /stats.
    fn stats(&self) -> Vec<(&'static str, i64)> {
        Vec::new()
    }
}
