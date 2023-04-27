use std::str::FromStr;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pulser::api::{PAIdent, PAVol, VolumeSpec};
use serde::Serialize;

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Info,
    List(ListArgs),

    GetSinkMute(BaseArgs),
    SetSinkMute(SetMuteArgs),
    GetSinkVolume(BaseArgs),
    SetSinkVolume(SetVolumeArgs),

    GetSourceMute(BaseArgs),
    SetSourceMute(SetMuteArgs),
    GetSourceVolume(BaseArgs),
    SetSourceVolume(SetVolumeArgs),
    // TODO: others...
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cards,
    Clients,
    Modules,
    Samples,
    Sinks,
    SinkInputs,
    Sources,
    SourceOutputs,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    // TODO: return CLI error if there are duplicates, currently not possible with clap
    // see: https://github.com/clap-rs/clap/discussions/4863
    /// Which objects you want to list. If you pass none, all objects will be listed.
    #[arg(value_enum)]
    pub kinds: Vec<Kind>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct BaseArgs {
    #[arg(long)]
    pub index: Option<u32>,
    #[arg(long)]
    pub name: Option<String>,
}

impl From<&BaseArgs> for PAIdent {
    fn from(value: &BaseArgs) -> Self {
        match (value.index, &value.name) {
            (Some(idx), None) => PAIdent::Index(idx),
            (None, Some(name)) => PAIdent::Name(name.clone()),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Bool {
    Yes,
    No,
    On,
    Off,
    True,
    False,
}

impl From<Bool> for bool {
    fn from(value: Bool) -> Self {
        use Bool::*;
        match value {
            Yes | On | True => true,
            No | Off | False => false,
        }
    }
}

#[derive(Debug, Args)]
pub struct SetMuteArgs {
    #[clap(flatten)]
    pub base_args: BaseArgs,
    #[arg(value_enum)]
    pub mute: Bool,
}

impl From<&SetMuteArgs> for PAIdent {
    fn from(value: &SetMuteArgs) -> Self {
        (&value.base_args).into()
    }
}

#[derive(Debug, Args)]
pub struct SetVolumeArgs {
    #[clap(flatten)]
    pub base_args: BaseArgs,
    /// A list of volumes. If only a single volume is provided, it is set for all channels of the
    /// object. If more are provided, the number must match the number of channels of the object.
    /// Provide the volume, in one of the following formats:
    /// "<INT>" (integer), "<INT|FLOAT>%" (percentage), "<FLOAT>dB" (decibels) or "<FLOAT>L" (linear)
    #[clap(required = true, num_args = 1.., value_parser = vol_from_str)]
    pub volumes: Vec<PAVol>,
}

impl From<&SetVolumeArgs> for PAIdent {
    fn from(value: &SetVolumeArgs) -> Self {
        (&value.base_args).into()
    }
}

impl From<&SetVolumeArgs> for VolumeSpec {
    fn from(value: &SetVolumeArgs) -> VolumeSpec {
        match value.volumes.len() {
            0 => unreachable!(),
            1 => VolumeSpec::All(value.volumes[0]),
            _ => VolumeSpec::Channels(value.volumes.clone()),
        }
    }
}

fn vol_from_str(s: &str) -> Result<PAVol, String> {
    PAVol::from_str(s).map_err(|e| e.to_string())
}
