use std::str::FromStr;

use clap::{Args, Parser, Subcommand};
use pulser::api::{PAIdent, PAVol, VolumeSpec};

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Info,

    // TODO: args for list, to filter on types
    List,

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

pub trait ToPAIdent {
    fn pa_ident(&self) -> PAIdent;
}

pub trait ToVolumeSpec {
    fn vol_spec(&self) -> VolumeSpec;
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct BaseArgs {
    #[arg(long)]
    pub index: Option<u32>,
    #[arg(long)]
    pub name: Option<String>,
}

impl ToPAIdent for BaseArgs {
    fn pa_ident(&self) -> PAIdent {
        match (self.index, &self.name) {
            (Some(idx), None) => PAIdent::Index(idx),
            (None, Some(name)) => PAIdent::Name(name.clone()),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Subcommand)]
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
    #[command(subcommand)]
    pub mute: Bool,
}

impl ToPAIdent for SetMuteArgs {
    fn pa_ident(&self) -> PAIdent {
        self.base_args.pa_ident()
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
    #[clap(required = true, num_args = 1.., value_parser = x)]
    pub volumes: Vec<PAVol>,
}

impl ToPAIdent for SetVolumeArgs {
    fn pa_ident(&self) -> PAIdent {
        self.base_args.pa_ident()
    }
}

impl ToVolumeSpec for SetVolumeArgs {
    fn vol_spec(&self) -> VolumeSpec {
        match self.volumes.len() {
            0 => unreachable!(),
            1 => VolumeSpec::All(self.volumes[0]),
            _ => VolumeSpec::Channels(self.volumes.clone()),
        }
    }
}

fn x(s: &str) -> Result<PAVol, String> {
    PAVol::from_str(s).map_err(|e| e.to_string())
}
