use libpulse_binding::channelmap::{self, Position};
pub use libpulse_binding::context::subscribe::Facility as Kind;
use libpulse_binding::sample;

#[derive(Debug)]
pub struct VolumeReading {
    /// Which channel this volume belongs to
    pub channel: Position,
    /// Volume as a percentage
    pub percentage: f64,
    /// Volume as a linear factor
    pub linear: f64,
    /// Volume actual value (`pa_volume_t`)
    pub value: u32,
    /// Volume in decibels
    pub db: f64,
}

#[derive(Debug, Clone)]
pub enum PAIdent {
    Index(u32),
    Name(String),
}

#[derive(Debug)]
pub enum PACommand {
    GetServerInfo,
    GetVolume(Kind, PAIdent),
    SetVolume, // TODO: volume struct?
    GetMute(Kind, PAIdent),
    SetMute(Kind, PAIdent),
}

#[derive(Debug)]
pub enum PAEvent {
    New(Kind, PAIdent),
    Removed(Kind, PAIdent),
    Changed(Kind, PAIdent),
    ServerInfo(PAServerInfo),
    Volume(PAIdent, Vec<VolumeReading>),
    Mute(PAIdent, bool),
}

#[derive(Debug)]
pub struct PAServerInfo {
    /// User name of the daemon process.
    pub user_name: Option<String>,
    /// Host name the daemon is running on.
    pub host_name: Option<String>,
    /// Version string of the daemon.
    pub server_version: Option<String>,
    /// Server package name (usually “pulseaudio”).
    pub server_name: Option<String>,
    /// Default sample specification.
    pub sample_spec: sample::Spec,
    /// Name of default sink.
    pub default_sink_name: Option<String>,
    /// Name of default source.
    pub default_source_name: Option<String>,
    /// A random cookie for identifying this instance of PulseAudio.
    pub cookie: u32,
    /// Default channel map.
    pub channel_map: channelmap::Map,
}
