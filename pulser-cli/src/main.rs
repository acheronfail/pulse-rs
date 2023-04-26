mod cli;

use std::error::Error;

use clap::Parser;
use cli::ToVolumeSpec;
use pulser::api::*;
use pulser::connect::PulseAudio;

use crate::cli::{Cli, Command, ToPAIdent};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let (tx, rx) = PulseAudio::connect("pulser");
    match args.command {
        Command::Info => {
            tx.send(PACommand::GetServerInfo)?;
            dbg!(rx.recv()?);
        }
        Command::GetSinkMute(args) => {
            tx.send(PACommand::GetSinkMute(args.pa_ident()))?;
            dbg!(rx.recv()?);
        }
        Command::GetSinkVolume(args) => {
            tx.send(PACommand::GetSinkVolume(args.pa_ident()))?;
            dbg!(rx.recv()?);
        }
        Command::SetSinkMute(args) => {
            tx.send(PACommand::SetSinkMute(args.pa_ident(), args.mute.into()))?;
            dbg!(rx.recv()?);
        }
        Command::SetSinkVolume(args) => {
            tx.send(PACommand::SetSinkVolume(args.pa_ident(), args.vol_spec()))?;
            dbg!(rx.recv()?);
        }
    };

    Ok(())
}
