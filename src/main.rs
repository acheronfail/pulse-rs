#![feature(never_type)]

pub mod pulseaudio;

use std::error::Error;
use std::sync::mpsc;
use std::thread;

// - [x] get source/sink info
// - [x] subscribe to source/sink changes
// - [x] mute source/sink
// - [x] change volume of source/sink
// - [x] perform change (vol/mute) without blocking on event loop
// - [ ] integrate with staturs (just spawn on thread I think)
// - [ ] do same MVP with pipewire-rs
// FIXME: I think I need to use the threaded main loop:
//  Assertion 'e->mainloop->n_enabled_defer_events > 0' failed at ../pulseaudio/src/pulse/mainloop.c:261, function mainloop_defer_enable(). Aborting.
//  See: https://github.com/jantap/rsmixer for inspiration on a threaded event loop
//  TODO: convert this to a lib (like `pulsectl`) and make it easy to use? publish to crates?
fn main() -> Result<(), Box<dyn Error>> {
    use libpulse_binding::context::subscribe::Facility;
    use pulseaudio::api::{PACommand, PAEvent, PAIdent};
    use pulseaudio::connect::PulseAudio;

    let (result_tx, result_rx) = mpsc::channel();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    thread::spawn(move || {
        if let Err(e) = PulseAudio::connect("MyAppName", cmd_rx, result_tx) {
            panic!("An error occurred while interfacing with pulseaudio: {}", e);
        }
    });

    loop {
        match result_rx.recv()? {
            PAEvent::ServerInfo(info) => {
                let sink_name = info.default_sink_name.unwrap();

                cmd_tx.send(PACommand::GetMute(
                    Facility::Sink,
                    PAIdent::Name(sink_name.clone()),
                ))?;

                cmd_tx.send(PACommand::GetVolume(
                    Facility::Sink,
                    PAIdent::Name(sink_name.clone()),
                ))?;
            }
            result => {
                dbg!(result);
            }
        }
    }
}
