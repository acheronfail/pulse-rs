pub mod command;
pub mod volume;

pub use command::*;
use libpulse_binding::context::introspect::{
    ServerInfo, SinkInfo, SinkPortInfo, SourceInfo, SourcePortInfo
};
use libpulse_binding::proplist::Proplist;
use libpulse_binding::time::MicroSeconds;
use libpulse_binding::volume::{ChannelVolumes, Volume};
use libpulse_binding::{channelmap, def, format, sample};
pub use volume::*;

#[derive(Debug, Clone)]
pub enum PAIdent {
    Index(u32),
    Name(String),
}

// TODO: there's probably a better way of doing this...
// See: https://github.com/jnqnfe/pulse-binding-rust/issues/44

#[derive(Debug, Clone)]
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

impl<'a> From<&'a ServerInfo<'a>> for PAServerInfo {
    fn from(value: &ServerInfo) -> Self {
        PAServerInfo {
            user_name: value.user_name.as_ref().map(|cow| cow.to_string()),
            host_name: value.host_name.as_ref().map(|cow| cow.to_string()),
            server_version: value.server_version.as_ref().map(|cow| cow.to_string()),
            server_name: value.server_name.as_ref().map(|cow| cow.to_string()),
            sample_spec: value.sample_spec,
            default_sink_name: value.default_sink_name.as_ref().map(|cow| cow.to_string()),
            default_source_name: value
                .default_source_name
                .as_ref()
                .map(|cow| cow.to_string()),
            cookie: value.cookie,
            channel_map: value.channel_map,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PASinkPortInfo {
    /// Name of this port.
    pub name: Option<String>,
    /// Description of this port.
    pub description: Option<String>,
    /// The higher this value is, the more useful this port is as a default.
    pub priority: u32,
    /// A flag indicating availability status of this port.
    pub available: def::PortAvailable,
}

impl<'a> From<&'a SinkPortInfo<'a>> for PASinkPortInfo {
    fn from(value: &'a SinkPortInfo<'a>) -> Self {
        PASinkPortInfo {
            name: value.name.as_ref().map(|cow| cow.to_string()),
            description: value.description.as_ref().map(|cow| cow.to_string()),
            priority: value.priority,
            available: value.available,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PASinkInfo {
    /// Name of the sink.
    pub name: Option<String>,
    /// Index of the sink.
    pub index: u32,
    /// Description of this sink.
    pub description: Option<String>,
    /// Sample spec of this sink.
    pub sample_spec: sample::Spec,
    /// Channel map.
    pub channel_map: channelmap::Map,
    /// Index of the owning module of this sink, or `None` if is invalid.
    pub owner_module: Option<u32>,
    /// Volume of the sink.
    pub volume: ChannelVolumes,
    /// Mute switch of the sink.
    pub mute: bool,
    /// Index of the monitor source connected to this sink.
    pub monitor_source: u32,
    /// The name of the monitor source.
    pub monitor_source_name: Option<String>,
    /// Length of queued audio in the output buffer.
    pub latency: MicroSeconds,
    /// Driver name.
    pub driver: Option<String>,
    /// Flags.
    pub flags: def::SinkFlagSet,
    /// Property list.
    pub proplist: Proplist,
    /// The latency this device has been configured to.
    pub configured_latency: MicroSeconds,
    /// Some kind of “base” volume that refers to unamplified/unattenuated volume in the context of
    /// the output device.
    pub base_volume: Volume,
    /// State.
    pub state: def::SinkState,
    /// Number of volume steps for sinks which do not support arbitrary volumes.
    pub n_volume_steps: u32,
    /// Card index, or `None` if invalid.
    pub card: Option<u32>,
    /// Set of available ports.
    pub ports: Vec<PASinkPortInfo>,
    /// Pointer to active port in the set, or `None`.
    pub active_port: Option<PASinkPortInfo>,
    /// Set of formats supported by the sink.
    pub formats: Vec<format::Info>,
}

impl<'a> From<&'a SinkInfo<'a>> for PASinkInfo {
    fn from(value: &'a SinkInfo<'a>) -> Self {
        PASinkInfo {
            name: value.name.as_ref().map(|cow| cow.to_string()),
            index: value.index,
            description: value.description.as_ref().map(|cow| cow.to_string()),
            sample_spec: value.sample_spec,
            channel_map: value.channel_map,
            owner_module: value.owner_module,
            volume: value.volume,
            mute: value.mute,
            monitor_source: value.monitor_source,
            monitor_source_name: value
                .monitor_source_name
                .as_ref()
                .map(|cow| cow.to_string()),
            latency: value.latency,
            driver: value.driver.as_ref().map(|cow| cow.to_string()),
            flags: value.flags,
            proplist: value.proplist.clone(),
            configured_latency: value.configured_latency,
            base_volume: value.base_volume,
            state: value.state,
            n_volume_steps: value.n_volume_steps,
            card: value.card,
            ports: value.ports.iter().map(|p| p.into()).collect(),
            active_port: value.active_port.as_ref().map(|p| (&**p).into()),
            formats: value.formats.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PASourcePortInfo {
    /// Name of this port.
    pub name: Option<String>,
    /// Description of this port.
    pub description: Option<String>,
    /// The higher this value is, the more useful this port is as a default.
    pub priority: u32,
    /// A flag indicating availability status of this port.
    pub available: def::PortAvailable,
}

impl<'a> From<&'a SourcePortInfo<'a>> for PASourcePortInfo {
    fn from(value: &'a SourcePortInfo<'a>) -> Self {
        PASourcePortInfo {
            name: value.name.as_ref().map(|cow| cow.to_string()),
            description: value.description.as_ref().map(|cow| cow.to_string()),
            priority: value.priority,
            available: value.available,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PASourceInfo {
    /// Name of the source.
    pub name: Option<String>,
    /// Index of the source.
    pub index: u32,
    /// Description of this source.
    pub description: Option<String>,
    /// Sample spec of this source.
    pub sample_spec: sample::Spec,
    /// Channel map.
    pub channel_map: channelmap::Map,
    /// Owning module index, or `None`.
    pub owner_module: Option<u32>,
    /// Volume of the source.
    pub volume: ChannelVolumes,
    /// Mute switch of the sink.
    pub mute: bool,
    /// If this is a monitor source, the index of the owning sink, otherwise `None`.
    pub monitor_of_sink: Option<u32>,
    /// Name of the owning sink, or `None`.
    pub monitor_of_sink_name: Option<String>,
    /// Length of filled record buffer of this source.
    pub latency: MicroSeconds,
    /// Driver name.
    pub driver: Option<String>,
    /// Flags.
    pub flags: def::SourceFlagSet,
    /// Property list.
    pub proplist: Proplist,
    /// The latency this device has been configured to.
    pub configured_latency: MicroSeconds,
    /// Some kind of “base” volume that refers to unamplified/unattenuated volume in the context of
    /// the input device.
    pub base_volume: Volume,
    /// State.
    pub state: def::SourceState,
    /// Number of volume steps for sources which do not support arbitrary volumes.
    pub n_volume_steps: u32,
    /// Card index, or `None`.
    pub card: Option<u32>,
    /// Set of available ports.
    pub ports: Vec<PASourcePortInfo>,
    /// Pointer to active port in the set, or `None`.
    pub active_port: Option<PASourcePortInfo>,
    /// Set of formats supported by the sink.
    pub formats: Vec<format::Info>,
}

impl<'a> From<&'a SourceInfo<'a>> for PASourceInfo {
    fn from(value: &'a SourceInfo<'a>) -> Self {
        PASourceInfo {
            name: value.name.as_ref().map(|cow| cow.to_string()),
            index: value.index,
            description: value.description.as_ref().map(|cow| cow.to_string()),
            sample_spec: value.sample_spec,
            channel_map: value.channel_map,
            owner_module: value.owner_module,
            volume: value.volume,
            mute: value.mute,
            monitor_of_sink: value.monitor_of_sink,
            monitor_of_sink_name: value
                .monitor_of_sink_name
                .as_ref()
                .map(|cow| cow.to_string()),
            latency: value.latency,
            driver: value.driver.as_ref().map(|cow| cow.to_string()),
            flags: value.flags,
            proplist: value.proplist.clone(),
            configured_latency: value.configured_latency,
            base_volume: value.base_volume,
            state: value.state,
            n_volume_steps: value.n_volume_steps,
            card: value.card,
            ports: value.ports.iter().map(|p| p.into()).collect(),
            active_port: value.active_port.as_ref().map(|p| (&**p).into()),
            formats: value.formats.clone(),
        }
    }
}
