mod cli;

use std::error::Error;

use clap::Parser;
use cli::ToVolumeSpec;
use pulser::api::*;
use pulser::connect::PulseAudio;

use crate::cli::Command::*;
use crate::cli::{Cli, ToPAIdent};

macro_rules! extract_unsafe {
    ($thing:expr, $extraction:pat => $id:ident) => {
        match $thing {
            $extraction => $id,
            _ => panic!("Unexpected match"),
        }
    };
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let (tx, rx) = PulseAudio::connect("pulser");
    match args.command {
        Info => {
            tx.send(PACommand::GetServerInfo)?;
            dbg!(rx.recv()?);
        }

        List => {
            tx.send(PACommand::GetSinkInfoList)?;
            tx.send(PACommand::GetSourceInfoList)?;
            let sinks = extract_unsafe!(rx.recv()?, PAEvent::SinkInfoList(x) => x);
            let sources = extract_unsafe!(rx.recv()?, PAEvent::SourceInfoList(x) => x);
            dbg!(sinks);
            dbg!(sources);
        }

        GetSinkMute(args) => {
            tx.send(PACommand::GetSinkMute(args.pa_ident()))?;
            dbg!(rx.recv()?);
        }
        GetSinkVolume(args) => {
            tx.send(PACommand::GetSinkVolume(args.pa_ident()))?;
            dbg!(rx.recv()?);
        }
        SetSinkMute(args) => {
            tx.send(PACommand::SetSinkMute(args.pa_ident(), args.mute.into()))?;
            dbg!(rx.recv()?);
        }
        SetSinkVolume(args) => {
            tx.send(PACommand::SetSinkVolume(args.pa_ident(), args.vol_spec()))?;
            dbg!(rx.recv()?);
        }

        GetSourceMute(args) => {
            tx.send(PACommand::GetSourceMute(args.pa_ident()))?;
            dbg!(rx.recv()?);
        }
        GetSourceVolume(args) => {
            tx.send(PACommand::GetSourceVolume(args.pa_ident()))?;
            dbg!(rx.recv()?);
        }
        SetSourceMute(args) => {
            tx.send(PACommand::SetSourceMute(args.pa_ident(), args.mute.into()))?;
            dbg!(rx.recv()?);
        }
        SetSourceVolume(args) => {
            tx.send(PACommand::SetSourceVolume(args.pa_ident(), args.vol_spec()))?;
            dbg!(rx.recv()?);
        }
    };

    Ok(())
}
