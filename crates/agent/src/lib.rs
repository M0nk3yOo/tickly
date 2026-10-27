//! Tracking-Kern der Companion-App (ohne Oberfläche, deshalb testbar).
//!
//! Der [`Engine`] beobachtet das System über [`tickly_platform::Platform`], schreibt Ereignisse in eine
//! dauerhafte [`Outbox`] und sendet sie über einen [`Transport`] an den Server. Der Server entscheidet
//! über den Tageszustand; der Companion meldet nur, was er sieht.
pub mod engine;
pub mod http;
pub mod outbox;
pub mod protocol;

pub use engine::{Action, Engine, View};
pub use http::{pair, Credentials, HttpTransport};
pub use outbox::Outbox;
pub use protocol::{Policy, QueuedEvent, Reply, ServerState, Transport, TransportError};
