use libpulse_binding::context::subscribe::Facility;
use libpulse_binding::{channelmap, sample};

#[derive(Debug, Clone)]
pub enum Ident {
    Index(u32),
    Name(String)
}

#[derive(Debug)]
pub struct FacilityIdentifier {
    pub facility: Facility,
    pub index: u32,
}

impl FacilityIdentifier {
    pub fn new(facility: Facility, index: u32) -> FacilityIdentifier {
        FacilityIdentifier { facility, index }
    }
}

#[derive(Debug)]
pub enum PulseAudioCommand {
    GetServerInfo,
    GetVolume(Ident),
    SetVolume, // TODO: volume struct?
    GetMute(Ident),
    SetMute(Ident),
}

#[derive(Debug)]
pub enum PulseAudioCommandResult {
    ServerInfo(PulseAudioServerInfo),
    Volume, // TODO: volume struct?
    Mute(Ident, bool),
}

#[derive(Debug)]
pub struct PulseAudioServerInfo {
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
