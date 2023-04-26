#![feature(never_type)]

pub mod pulseaudio;

use std::error::Error;
use std::sync::mpsc;
use std::thread;

fn main() -> Result<(), Box<dyn Error>> {
    use pulseaudio::api::*;
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
                let default_sink_name = info.default_sink_name.unwrap();
                let sink_id = PAIdent::Name(default_sink_name.clone());
                cmd_tx.send(PACommand::GetSinkMute(sink_id.clone()))?;
                cmd_tx.send(PACommand::GetSinkVolume(sink_id.clone()))?;

                let default_source_name = info.default_source_name.unwrap();
                let source_id = PAIdent::Name(default_source_name.clone());
                cmd_tx.send(PACommand::SetSourceMute(source_id.clone(), false))?;
                cmd_tx.send(PACommand::SetSourceVolume(
                    source_id.clone(),
                    VolumeSpec::All(PAVol::Percentage(30.0)),
                ))?;
            }
            result => {
                dbg!(result);
            }
        }
    }
}
