mod cli;
mod subscribe;

use std::collections::BTreeMap;
use std::error::Error;

use clap::{Parser, ValueEnum};
use pulser::api::PAMask;
use pulser::simple::{OperationResult, PulseAudio};
use serde_json::{to_value, Value};

use crate::cli::Command::*;
use crate::cli::{Cli, Kind};

#[macro_export]
macro_rules! json_print {
    ($x:expr) => {
        println!("{}", serde_json::to_string(&$x)?)
    };
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let pa = PulseAudio::connect(Some("PulserCli"));
    match args.command {
        Info => {
            json_print!(pa.get_server_info()?);
        }
        GetDefaultSink => json_print!(pa.get_default_sink()?),
        GetDefaultSource => json_print!(pa.get_default_source()?),
        SetDefaultSink(args) => json_print!(pa.set_default_sink((&args).into())?),
        SetDefaultSource(args) => json_print!(pa.set_default_source((&args).into())?),

        List(args) => {
            // unfortunately can't dedup with clap, so we do that here and silently ignore duplicates
            let mut kinds = args.kinds;
            kinds.sort();
            kinds.dedup();

            let kinds = if kinds.len() == 0 {
                Kind::value_variants().to_vec()
            } else {
                kinds
            };

            // collect into a `BTreeMap` to have it sorted by key
            let map = kinds
                .into_iter()
                .map(|k| -> Result<(Kind, Value), Box<dyn Error>> {
                    Ok((
                        k,
                        match k {
                            Kind::Cards => to_value(pa.get_card_info_list()?)?,
                            Kind::Clients => to_value(pa.get_client_info_list()?)?,
                            Kind::Modules => to_value(pa.get_module_info_list()?)?,
                            Kind::Samples => to_value(pa.get_sample_info_list()?)?,
                            Kind::Sinks => to_value(pa.get_sink_info_list()?)?,
                            Kind::SinkInputs => to_value(pa.get_sink_input_info_list()?)?,
                            Kind::Sources => to_value(pa.get_source_info_list()?)?,
                            Kind::SourceOutputs => to_value(pa.get_source_output_info_list()?)?,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<Kind, _>, _>>()
                .unwrap();

            if map.len() == 1 {
                json_print!(map.values().next().unwrap());
            } else {
                json_print!(map);
            }
        }

        GetCardInfo(args) => json_print!(pa.get_card_info((&args).into())?),
        SetCardProfile(args) => {
            json_print!(pa.set_card_profile((&args.base_args).into(), args.profile)?)
        }
        SetPortLatencyOffset(args) => {
            json_print!(pa.set_port_latency_offset(args.card_id(), args.port_id(), args.offset)?)
        }

        GetClientInfo(args) => json_print!(pa.get_client_info((&args).into())?),
        KillClient(args) => json_print!(pa.kill_client((&args).into())?),

        GetModuleInfo(args) => json_print!(pa.get_module_info((&args).into())?),
        LoadModule(args) => json_print!(pa.load_module(args.name, args.args)?),
        UnloadModule(args) => json_print!(pa.unload_module((&args).into())?),

        GetSinkInfo(args) => json_print!(pa.get_sink_info((&args).into())?),
        GetSinkMute(args) => json_print!(pa.get_sink_mute((&args).into())?),
        GetSinkVolume(args) => json_print!(pa.get_sink_volume((&args).into())?),
        SetSinkMute(args) => {
            json_print!(pa.set_sink_mute((&args.base_args).into(), args.mute.into())?)
        }
        SetSinkVolume(args) => json_print!(pa.set_sink_volume((&args).into(), (&args).into())?),
        SetSinkPort(args) => json_print!(pa.set_sink_port((&args.base_args).into(), args.port)?),
        SuspendSink(args) => {
            json_print!(pa.suspend_sink((&args.base_args).into(), args.suspend.into())?)
        }

        GetSourceInfo(args) => json_print!(pa.get_source_info((&args).into())?),
        GetSourceMute(args) => json_print!(pa.get_source_mute((&args).into())?),
        GetSourceVolume(args) => json_print!(pa.get_source_volume((&args).into())?),
        SetSourceMute(args) => {
            json_print!(pa.set_source_mute((&args.base_args).into(), args.mute.into())?)
        }
        SetSourceVolume(args) => json_print!(pa.set_source_volume((&args).into(), (&args).into())?),
        SetSourcePort(args) => {
            json_print!(pa.set_source_port((&args.base_args).into(), args.port)?)
        }
        SuspendSource(args) => {
            json_print!(pa.suspend_source((&args.base_args).into(), args.suspend.into())?)
        }

        GetSinkInputInfo(args) => json_print!(pa.get_sink_input_info((&args).into())?),
        GetSinkInputMute(args) => json_print!(pa.get_sink_input_mute((&args).into())?),
        GetSinkInputVolume(args) => json_print!(pa.get_sink_input_volume((&args).into())?),
        SetSinkInputMute(args) => {
            json_print!(pa.set_sink_input_mute((&args.base_args).into(), args.mute.into())?)
        }
        SetSinkInputVolume(args) => {
            json_print!(pa.set_sink_input_volume((&args).into(), (&args).into())?)
        }
        MoveSinkInput(args) => json_print!(pa.move_sink_input(args.from_id(), args.to_id())?),
        KillSinkInput(args) => json_print!(pa.kill_sink_input((&args).into())?),

        GetSourceOutputInfo(args) => json_print!(pa.get_source_output_info((&args).into())?),
        GetSourceOutputMute(args) => json_print!(pa.get_source_output_mute((&args).into())?),
        GetSourceOutputVolume(args) => json_print!(pa.get_source_output_volume((&args).into())?),
        SetSourceOutputMute(args) => {
            json_print!(pa.set_source_output_mute((&args.base_args).into(), args.mute.into())?)
        }
        SetSourceOutputVolume(args) => {
            json_print!(pa.set_source_output_volume((&args).into(), (&args).into())?)
        }
        MoveSourceOutput(args) => json_print!(pa.move_source_output(args.from_id(), args.to_id())?),
        KillSourceOutput(args) => json_print!(pa.kill_source_output((&args).into())?),

        Subscribe(args) => {
            let mask = if args.kinds.is_empty() {
                PAMask::ALL
            } else {
                let mut mask = PAMask::empty();
                for kind in args.kinds {
                    mask.insert(match kind {
                        Kind::Cards => PAMask::CARD,
                        Kind::Clients => PAMask::CLIENT,
                        Kind::Modules => PAMask::MODULE,
                        Kind::Samples => PAMask::SAMPLE_CACHE,
                        Kind::Sinks => PAMask::SINK,
                        Kind::SinkInputs => PAMask::SINK_INPUT,
                        Kind::Sources => PAMask::SOURCE,
                        Kind::SourceOutputs => PAMask::SOURCE_OUTPUT,
                    });
                }

                mask
            };

            subscribe::subscribe(pa, mask)?;
        }
    };

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        println!(
            "{}",
            serde_json::to_string(&OperationResult::Failure {
                error: e.to_string(),
            })
            .unwrap_or_else(|e| format!("Failed to serialize error: {}", e))
        );
    }
}
