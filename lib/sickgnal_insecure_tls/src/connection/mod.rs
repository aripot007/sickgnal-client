//!
//! Manages the TLS state machine to send and receive data
//!
//! The connection is used to manage the TLS state machine ([`State`]),
//! and the [`Sender`] and [`Receiver`] used to encrypt / decrypt messages.
//!
//! [`State`]: state::State
//!
//! # Diagram
//!
//! The following diagram shows how the 3 components interact :
//!
//! Connection                         Sender                       
//!  ┌┐          ┌─────────────────────────────────────────────────┐
//!  ││ app data │                                                 │
//!  │├──────────┼─────────► fragment ──┐                          │
//!  ││          │ ┌───────► +encrypt   │             ┌──────────┐ │
//!  ││          │ │                    ├──► frame ──►│output buf│ │
//!  ││          │ │                    │             └─────┬────┘ │
//!  ││          │ │     ┌──────────────┘                   │      │
//!  ││          │ │     │                                  │      │
//!  ││          └─┼─────┼──────────────────────────────────┼──────┘
//!  ││            │     │         ▲                        │       
//!  ││         hs │     │ alerts  │ key update       write │       
//!  ││            │     │         │ requested              ▼       
//!  ││      ┌─────┴─────┴───┐     │                 ┌────────────┐
//!  ││      │               ├─────┘                 │            │
//!  ││      │ State machine │                       │ TCP socket │
//!  ││      │               ├───┐                   │            │
//!  ││      └───────────────┘   │ key updates       └──────┬─────┘
//!  ││            ▲    ▲        ▼                     read │       
//!  ││    ┌───────┼────┼───────────────────────────────────┼──────┐
//!  ││    │  alert│    │                 ┌─────────┐       ▼      │
//!  ││    │       │    └─── defragment ◄─┤hs defrag│ ┌─────────┐  │
//!  ││    │       │                      │   buf   │ │input buf│  │
//!  ││    │error  │                      └─────────┘ └────────┬┘  │
//!  ││◄───┼───────┴───────────────┐        ▲ hanshake         │   │
//!  ││    │                alert  │        │ fragment         │   │
//!  ││    │  ┌────────┐           │                           │   │
//!  ││◄───┼──┤data buf│◄──────────┴────── decrypt ◄─ deframe ◄┘   │
//!  ││    │  └────────┘ app data                                  │
//!  └┘    └───────────────────────────────────────────────────────┘
//!                                Receiver                         
//!

mod connection;
mod encryption_state;
pub(self) mod receiver;
pub(self) mod sender;
pub(self) mod state;
pub use connection::*;
