use std::str::FromStr;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pulser::api::{PAIdent, PAVol, VolumeSpec};
use serde::Serialize;

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

// TODO: think about a nice API for this... right now I'm just implementing things here
// as a way to help me implement more commands in the crate's library
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Get server information
    Info,
    /// List objects from the server
    List(ListArgs),

    /// Get the default sink (if any)
    GetDefaultSink,
    /// Get the default sink (if any)
    SetDefaultSink(BaseArgs),
    /// Get the default source (if any)
    GetDefaultSource,
    /// Get the default source (if any)
    SetDefaultSource(BaseArgs),

    /// Check if a sink is muted
    GetSinkMute(BaseArgs),
    /// Mute a sink
    SetSinkMute(SetMuteArgs),
    /// Get the volume from a sink
    GetSinkVolume(BaseArgs),
    /// Set the volume(s) for a sink
    SetSinkVolume(SetVolumeArgs),

    /// Check if a source is muted
    GetSourceMute(BaseArgs),
    /// Mute a source
    SetSourceMute(SetMuteArgs),
    /// Get the volume from a source
    GetSourceVolume(BaseArgs),
    /// Set the volume(s) for a source
    SetSourceVolume(SetVolumeArgs),

    /// Check if a sink-input is muted
    GetSinkInputMute(BaseArgs),
    /// Mute a sink-input
    SetSinkInputMute(SetMuteArgs),
    /// Get the volume from a sink-input
    GetSinkInputVolume(BaseArgs),
    /// Set the volume(s) for a sink-input
    SetSinkInputVolume(SetVolumeArgs),

    /// Check if a source-output is muted
    GetSourceOutputMute(BaseArgs),
    /// Mute a source-output
    SetSourceOutputMute(SetMuteArgs),
    /// Get the volume from a source-output
    GetSourceOutputVolume(BaseArgs),
    /// Set the volume(s) for a source-output
    SetSourceOutputVolume(SetVolumeArgs),

    /// Subscribe to server events
    Subscribe(SubscribeArgs),
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

#[derive(Debug, Args)]
pub struct SubscribeArgs {
    #[arg(value_enum)]
    pub kinds: Vec<Kind>,
}
