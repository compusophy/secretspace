//! A room: one game's world, hosted by the server in a thread of its own.
//! Browsers come and go and say things; every tick the room says what
//! each of them should be told. The server does the sockets: a browser
//! that cannot keep up is closed and the room is told, never waited on.

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
    /// A browser connected. A watcher only looks (the hub's live preview,
    /// say): it cannot play, and it is not one of the people here.
    fn open(&mut self, conn: u32, watch: bool, out: &mut Outbox);
    /// A browser said something (untrusted bytes).
    fn message(&mut self, conn: u32, bytes: &[u8], out: &mut Outbox);
    /// A browser left, or was let go.
    fn close(&mut self, conn: u32);
    /// One tick of the world.
    fn tick(&mut self, out: &mut Outbox);
    /// People here now, watchers aside.
    fn people(&self) -> usize;
}
