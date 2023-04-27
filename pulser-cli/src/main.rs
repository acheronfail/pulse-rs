mod cli;

use std::collections::HashMap;
use std::error::Error;

use clap::{Parser, ValueEnum};
use itertools::Itertools;
use pulser::api::*;
use pulser::connect::PulseAudio;

use crate::cli::Command::*;
use crate::cli::{Cli, Kind};

macro_rules! json_print {
    ($x:expr) => {
        println!("{}", serde_json::to_string(&$x)?);
    };
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let (tx, rx) = PulseAudio::connect("pulser");
    match args.command {
        Info => {
            tx.send(PACommand::GetServerInfo)?;
            json_print!(rx.recv()?);
        }
        List(args) => {
            // unfortunately can't dedup with clap, and using `Vec::dedup` requires
            // sorting the list, which we don't want to do here (since it's nice to retain
            // the order given on the command line)
            let kinds: Vec<Kind> = args.kinds.into_iter().unique().collect();
            let kinds = if kinds.len() == 0 {
                Kind::value_variants().to_vec()
            } else {
                kinds
            };

            let map = kinds
                .into_iter()
                .map(|k| {
                    tx.send(match k {
                        Kind::Cards => PACommand::GetCardInfoList,
                        Kind::Clients => PACommand::GetClientInfoList,
                        Kind::Modules => PACommand::GetModuleInfoList,
                        Kind::Samples => PACommand::GetSampleInfoList,
                        Kind::Sinks => PACommand::GetSinkInfoList,
                        Kind::SinkInputs => PACommand::GetSinkInputInfoList,
                        Kind::Sources => PACommand::GetSourceInfoList,
                        Kind::SourceOutputs => PACommand::GetSourceOutputInfoList,
                    })
                    .unwrap();
                    (k, rx.recv().unwrap())
                })
                .collect::<HashMap<Kind, PAEvent>>();

            json_print!(map);
        }
        GetSinkMute(args) => {
            tx.send(PACommand::GetSinkMute((&args).into()))?;
            json_print!(rx.recv()?);
        }
        GetSinkVolume(args) => {
            tx.send(PACommand::GetSinkVolume((&args).into()))?;
            json_print!(rx.recv()?);
        }
        SetSinkMute(args) => {
            tx.send(PACommand::SetSinkMute((&args).into(), args.mute.into()))?;
            json_print!(rx.recv()?);
        }
        SetSinkVolume(args) => {
            tx.send(PACommand::SetSinkVolume((&args).into(), (&args).into()))?;
            json_print!(rx.recv()?);
        }
        GetSourceMute(args) => {
            tx.send(PACommand::GetSourceMute((&args).into()))?;
            json_print!(rx.recv()?);
        }
        GetSourceVolume(args) => {
            tx.send(PACommand::GetSourceVolume((&args).into()))?;
            json_print!(rx.recv()?);
        }
        SetSourceMute(args) => {
            tx.send(PACommand::SetSourceMute((&args).into(), args.mute.into()))?;
            json_print!(rx.recv()?);
        }
        SetSourceVolume(args) => {
            tx.send(PACommand::SetSourceVolume((&args).into(), (&args).into()))?;
            json_print!(rx.recv()?);
        }
    };

    Ok(())
}
