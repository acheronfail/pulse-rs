//! HACK: This file (and all the structs within it) exists because the internal types provided by
//! libpulse_binding aren't `Clone` or `Copy`, and worse, are borrowed structs which are only valid
//! within the api callback. That means we can't easily copy them in Rust, because of borrow semantics.
//!
//! The structs are copies of libpulse_binding's structs, but have `Clone` implemented and also don't
//! have borrow any types.
//! For more, see: https://github.com/jnqnfe/pulse-binding-rust/issues/44
//!
//! TODO: see if there's a way to automate this (proc macro? hacky script?)
//! TODO: these structs are currently missing any fields that are gated behind feature flags

use libpulse_binding::channelmap::Position;
use libpulse_binding::context::introspect::{
    CardInfo,
    CardPortInfo,
    ClientInfo,
    ModuleInfo,
    SampleInfo,
    ServerInfo,
    SinkInfo,
    SinkInputInfo,
    SinkPortInfo,
    SourceInfo,
    SourceOutputInfo,
    SourcePortInfo,
};
use libpulse_binding::context::subscribe::Facility;
use libpulse_binding::proplist::Proplist;
use libpulse_binding::time::MicroSeconds;
use libpulse_binding::volume::{ChannelVolumes, Volume, VolumeDB, VolumeLinear};
use libpulse_binding::{channelmap, def, direction, format, sample};
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

macro_rules! cow {
    ($cow:expr) => {
        $cow.as_ref().map(|p| (&**p).into())
    };
}

fn ser_sample_spec<S>(sample_sec: &sample::Spec, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = s.serialize_map(None)?;
    map.serialize_entry("channels", &sample_sec.channels)?;
    map.serialize_entry("rate", &sample_sec.rate)?;
    map.serialize_entry("format", &sample_sec.format.to_string())?;
    map.end()
}

fn ser_port_available<S>(available: &def::PortAvailable, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i8(*available as i8)
}

fn ser_channel_volumes<S>(volume: &ChannelVolumes, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = s.serialize_map(None)?;
    map.serialize_entry("channels", &volume.len())?;
    map.serialize_entry(
        "volumes",
        &volume
            .get()
            .iter()
            .map(|v| PAVolume::from(*v))
            .collect::<Vec<_>>(),
    )?;
    map.end()
}

fn ser_microseconds<S>(latency: &MicroSeconds, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u64(latency.0)
}

fn ser_sink_flag_set<S>(flags: &def::SinkFlagSet, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u32(flags.bits())
}

fn ser_source_flag_set<S>(flags: &def::SourceFlagSet, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u32(flags.bits())
}

fn ser_sink_state<S>(state: &def::SinkState, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i8(*state as i8)
}

fn ser_source_state<S>(state: &def::SourceState, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i8(*state as i8)
}

fn ser_flag_set<S>(formats: &direction::FlagSet, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i32(formats.bits())
}

#[derive(Debug, Clone, Serialize)]
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
    #[serde(serialize_with = "ser_sample_spec")]
    pub sample_spec: sample::Spec,
    /// Name of default sink.
    pub default_sink_name: Option<String>,
    /// Name of default source.
    pub default_source_name: Option<String>,
    /// A random cookie for identifying this instance of PulseAudio.
    pub cookie: u32,
    /// Default channel map.
    pub channel_map: PAChannelMap,
}

impl<'a> From<&'a ServerInfo<'a>> for PAServerInfo {
    fn from(value: &ServerInfo) -> Self {
        PAServerInfo {
            user_name: cow!(value.user_name),
            host_name: cow!(value.host_name),
            server_version: cow!(value.server_version),
            server_name: cow!(value.server_name),
            sample_spec: value.sample_spec,
            default_sink_name: cow!(value.default_sink_name),
            default_source_name: value
                .default_source_name
                .as_ref()
                .map(|cow| cow.to_string()),
            cookie: value.cookie,
            channel_map: value.channel_map.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASinkPortInfo {
    /// Name of this port.
    pub name: Option<String>,
    /// Description of this port.
    pub description: Option<String>,
    /// The higher this value is, the more useful this port is as a default.
    pub priority: u32,
    /// A flag indicating availability status of this port.
    #[serde(serialize_with = "ser_port_available")]
    pub available: def::PortAvailable,
}

impl<'a> From<&'a SinkPortInfo<'a>> for PASinkPortInfo {
    fn from(value: &'a SinkPortInfo<'a>) -> Self {
        PASinkPortInfo {
            name: cow!(value.name),
            description: cow!(value.description),
            priority: value.priority,
            available: value.available,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASinkInfo {
    /// Name of the sink.
    pub name: Option<String>,
    /// Index of the sink.
    pub index: u32,
    /// Description of this sink.
    pub description: Option<String>,
    /// Sample spec of this sink.
    #[serde(serialize_with = "ser_sample_spec")]
    pub sample_spec: sample::Spec,
    /// Channel map.
    pub channel_map: PAChannelMap,
    /// Index of the owning module of this sink, or `None` if is invalid.
    pub owner_module: Option<u32>,
    /// Volume of the sink.
    #[serde(serialize_with = "ser_channel_volumes")]
    pub volume: ChannelVolumes,
    /// Mute switch of the sink.
    pub mute: bool,
    /// Index of the monitor source connected to this sink.
    pub monitor_source: u32,
    /// The name of the monitor source.
    pub monitor_source_name: Option<String>,
    /// Length of queued audio in the output buffer.
    #[serde(serialize_with = "ser_microseconds")]
    pub latency: MicroSeconds,
    /// Driver name.
    pub driver: Option<String>,
    /// Flags.
    #[serde(serialize_with = "ser_sink_flag_set")]
    pub flags: def::SinkFlagSet,
    /// Property list.
    pub proplist: PAProplist,
    /// The latency this device has been configured to.
    #[serde(serialize_with = "ser_microseconds")]
    pub configured_latency: MicroSeconds,
    /// Some kind of “base” volume that refers to unamplified/unattenuated volume in the context of
    /// the output device.
    pub base_volume: PAVolume,
    /// State.
    #[serde(serialize_with = "ser_sink_state")]
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
    pub formats: Vec<PAInfo>,
}

impl<'a> From<&'a SinkInfo<'a>> for PASinkInfo {
    fn from(value: &'a SinkInfo<'a>) -> Self {
        PASinkInfo {
            name: cow!(value.name),
            index: value.index,
            description: cow!(value.description),
            sample_spec: value.sample_spec,
            channel_map: value.channel_map.into(),
            owner_module: value.owner_module,
            volume: value.volume,
            mute: value.mute,
            monitor_source: value.monitor_source,
            monitor_source_name: value
                .monitor_source_name
                .as_ref()
                .map(|cow| cow.to_string()),
            latency: value.latency,
            driver: cow!(value.driver),
            flags: value.flags,
            proplist: value.proplist.clone().into(),
            configured_latency: value.configured_latency,
            base_volume: value.base_volume.into(),
            state: value.state,
            n_volume_steps: value.n_volume_steps,
            card: value.card,
            ports: value.ports.iter().map(|p| p.into()).collect(),
            active_port: value.active_port.as_ref().map(|p| (&**p).into()),
            formats: value.formats.iter().map(|i| PAInfo(i.clone())).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASourcePortInfo {
    /// Name of this port.
    pub name: Option<String>,
    /// Description of this port.
    pub description: Option<String>,
    /// The higher this value is, the more useful this port is as a default.
    pub priority: u32,
    /// A flag indicating availability status of this port.
    #[serde(serialize_with = "ser_port_available")]
    pub available: def::PortAvailable,
}

impl<'a> From<&'a SourcePortInfo<'a>> for PASourcePortInfo {
    fn from(value: &'a SourcePortInfo<'a>) -> Self {
        PASourcePortInfo {
            name: cow!(value.name),
            description: cow!(value.description),
            priority: value.priority,
            available: value.available,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASourceInfo {
    /// Name of the source.
    pub name: Option<String>,
    /// Index of the source.
    pub index: u32,
    /// Description of this source.
    pub description: Option<String>,
    /// Sample spec of this source.
    #[serde(serialize_with = "ser_sample_spec")]
    pub sample_spec: sample::Spec,
    /// Channel map.
    pub channel_map: PAChannelMap,
    /// Owning module index, or `None`.
    pub owner_module: Option<u32>,
    /// Volume of the source.
    #[serde(serialize_with = "ser_channel_volumes")]
    pub volume: ChannelVolumes,
    /// Mute switch of the sink.
    pub mute: bool,
    /// If this is a monitor source, the index of the owning sink, otherwise `None`.
    pub monitor_of_sink: Option<u32>,
    /// Name of the owning sink, or `None`.
    pub monitor_of_sink_name: Option<String>,
    /// Length of filled record buffer of this source.
    #[serde(serialize_with = "ser_microseconds")]
    pub latency: MicroSeconds,
    /// Driver name.
    pub driver: Option<String>,
    /// Flags.
    #[serde(serialize_with = "ser_source_flag_set")]
    pub flags: def::SourceFlagSet,
    /// Property list.
    pub proplist: PAProplist,
    /// The latency this device has been configured to.
    #[serde(serialize_with = "ser_microseconds")]
    pub configured_latency: MicroSeconds,
    /// Some kind of “base” volume that refers to unamplified/unattenuated volume in the context of
    /// the input device.
    pub base_volume: PAVolume,
    /// State.
    #[serde(serialize_with = "ser_source_state")]
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
    pub formats: Vec<PAInfo>,
}

impl<'a> From<&'a SourceInfo<'a>> for PASourceInfo {
    fn from(value: &'a SourceInfo<'a>) -> Self {
        PASourceInfo {
            name: cow!(value.name),
            index: value.index,
            description: cow!(value.description),
            sample_spec: value.sample_spec,
            channel_map: value.channel_map.into(),
            owner_module: value.owner_module,
            volume: value.volume,
            mute: value.mute,
            monitor_of_sink: value.monitor_of_sink,
            monitor_of_sink_name: value
                .monitor_of_sink_name
                .as_ref()
                .map(|cow| cow.to_string()),
            latency: value.latency,
            driver: cow!(value.driver),
            flags: value.flags,
            proplist: value.proplist.clone().into(),
            configured_latency: value.configured_latency,
            base_volume: value.base_volume.into(),
            state: value.state,
            n_volume_steps: value.n_volume_steps,
            card: value.card,
            ports: value.ports.iter().map(|p| p.into()).collect(),
            active_port: value.active_port.as_ref().map(|p| (&**p).into()),
            formats: value.formats.iter().map(|i| PAInfo(i.clone())).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASinkInputInfo {
    /// Index of the sink input.
    pub index: u32,
    /// Name of the sink input.
    pub name: Option<String>,
    /// Index of the module this sink input belongs to, or `None` when it does not belong to any
    /// module.
    pub owner_module: Option<u32>,
    /// Index of the client this sink input belongs to, or invalid when it does not belong to any
    /// client.
    pub client: Option<u32>,
    /// Index of the connected sink.
    pub sink: u32,
    /// The sample specification of the sink input.
    #[serde(serialize_with = "ser_sample_spec")]
    pub sample_spec: sample::Spec,
    /// Channel map.
    pub channel_map: PAChannelMap,
    /// The volume of this sink input.
    #[serde(serialize_with = "ser_channel_volumes")]
    pub volume: ChannelVolumes,
    /// Latency due to buffering in sink input, see [`TimingInfo`](crate::def::TimingInfo) for
    /// details.
    #[serde(serialize_with = "ser_microseconds")]
    pub buffer_usec: MicroSeconds,
    /// Latency of the sink device, see [`TimingInfo`](crate::def::TimingInfo) for details.
    #[serde(serialize_with = "ser_microseconds")]
    pub sink_usec: MicroSeconds,
    /// The resampling method used by this sink input.
    pub resample_method: Option<String>,
    /// Driver name.
    pub driver: Option<String>,
    /// Stream muted.
    pub mute: bool,
    /// Property list.
    pub proplist: PAProplist,
    /// Stream corked.
    pub corked: bool,
    /// Stream has volume. If not set, then the meaning of this struct’s volume member is
    /// unspecified.
    pub has_volume: bool,
    /// The volume can be set. If not set, the volume can still change even though clients can’t
    /// control the volume.
    pub volume_writable: bool,
    /// Stream format information.
    pub format: PAInfo,
}

impl<'a> From<&'a SinkInputInfo<'a>> for PASinkInputInfo {
    fn from(value: &'a SinkInputInfo<'a>) -> Self {
        PASinkInputInfo {
            index: value.index,
            name: cow!(value.name),
            owner_module: value.owner_module,
            client: value.client,
            sink: value.sink,
            sample_spec: value.sample_spec,
            channel_map: value.channel_map.into(),
            volume: value.volume,
            buffer_usec: value.buffer_usec,
            sink_usec: value.sink_usec,
            resample_method: cow!(value.resample_method),
            driver: cow!(value.driver),
            mute: value.mute,
            proplist: value.proplist.clone().into(),
            corked: value.corked,
            has_volume: value.has_volume,
            volume_writable: value.volume_writable,
            format: value.format.clone().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASourceOutputInfo {
    /// Index of the source output.
    pub index: u32,
    /// Name of the source output.
    pub name: Option<String>,
    /// Index of the module this source output belongs to, or `None` when it does not belong to any
    /// module.
    pub owner_module: Option<u32>,
    /// Index of the client this source output belongs to, or `None` when it does not belong to any
    /// client.
    pub client: Option<u32>,
    /// Index of the connected source.
    pub source: u32,
    /// The sample specification of the source output.
    #[serde(serialize_with = "ser_sample_spec")]
    pub sample_spec: sample::Spec,
    /// Channel map.
    pub channel_map: PAChannelMap,
    /// Latency due to buffering in the source output, see [`TimingInfo`](crate::def::TimingInfo)
    /// for details.
    #[serde(serialize_with = "ser_microseconds")]
    pub buffer_usec: MicroSeconds,
    /// Latency of the source device, see [`TimingInfo`](crate::def::TimingInfo) for details.
    #[serde(serialize_with = "ser_microseconds")]
    pub source_usec: MicroSeconds,
    /// The resampling method used by this source output.
    pub resample_method: Option<String>,
    /// Driver name.
    pub driver: Option<String>,
    /// Property list.
    pub proplist: PAProplist,
    /// Stream corked.
    pub corked: bool,
    /// The volume of this source output.
    #[serde(serialize_with = "ser_channel_volumes")]
    pub volume: ChannelVolumes,
    /// Stream muted.
    pub mute: bool,
    /// Stream has volume. If not set, then the meaning of this struct’s volume member is
    /// unspecified.
    pub has_volume: bool,
    /// The volume can be set. If not set, the volume can still change even though clients can’t
    /// control the volume.
    pub volume_writable: bool,
    /// Stream format information.
    pub format: PAInfo,
}

impl<'a> From<&'a SourceOutputInfo<'a>> for PASourceOutputInfo {
    fn from(value: &'a SourceOutputInfo<'a>) -> Self {
        PASourceOutputInfo {
            index: value.index,
            name: value.name.as_ref().map(|p| (&**p).into()),
            owner_module: value.owner_module,
            client: value.client,
            source: value.source,
            sample_spec: value.sample_spec,
            channel_map: value.channel_map.into(),
            buffer_usec: value.buffer_usec,
            source_usec: value.source_usec,
            resample_method: value.resample_method.as_ref().map(|p| (&**p).into()),
            driver: value.driver.as_ref().map(|p| (&**p).into()),
            proplist: value.proplist.clone().into(),
            corked: value.corked,
            volume: value.volume,
            mute: value.mute,
            has_volume: value.has_volume,
            volume_writable: value.volume_writable,
            format: value.format.clone().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PAClientInfo {
    /// Index of this client.
    pub index: u32,
    /// Name of this client.
    pub name: Option<String>,
    /// Index of the owning module, or `None`.
    pub owner_module: Option<u32>,
    /// Driver name.
    pub driver: Option<String>,
    /// Property list.
    pub proplist: PAProplist,
}

impl<'a> From<&'a ClientInfo<'a>> for PAClientInfo {
    fn from(value: &'a ClientInfo<'a>) -> Self {
        PAClientInfo {
            index: value.index,
            name: value.name.as_ref().map(|p| (&**p).into()),
            owner_module: value.owner_module,
            driver: value.driver.as_ref().map(|p| (&**p).into()),
            proplist: value.proplist.clone().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PASampleInfo {
    /// Index of this entry.
    pub index: u32,
    /// Name of this entry.
    pub name: Option<String>,
    /// Default volume of this entry.
    #[serde(serialize_with = "ser_channel_volumes")]
    pub volume: ChannelVolumes,
    /// Sample specification of the sample.
    #[serde(serialize_with = "ser_sample_spec")]
    pub sample_spec: sample::Spec,
    /// The channel map.
    pub channel_map: PAChannelMap,
    /// Duration of this entry.
    #[serde(serialize_with = "ser_microseconds")]
    pub duration: MicroSeconds,
    /// Length of this sample in bytes.
    pub bytes: u32,
    /// Non-zero when this is a lazy cache entry.
    pub lazy: bool,
    /// In case this is a lazy cache entry, the filename for the sound file to be loaded on demand.
    pub filename: Option<String>,
    /// Property list for this sample.
    pub proplist: PAProplist,
}

impl<'a> From<&'a SampleInfo<'a>> for PASampleInfo {
    fn from(value: &'a SampleInfo<'a>) -> Self {
        PASampleInfo {
            index: value.index,
            name: cow!(value.name),
            volume: value.volume,
            sample_spec: value.sample_spec,
            channel_map: value.channel_map.into(),
            duration: value.duration,
            bytes: value.bytes,
            lazy: value.lazy,
            filename: cow!(value.filename),
            proplist: value.proplist.clone().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PACardPortInfo {
    /// Name of this port.
    pub name: Option<String>,
    /// Description of this port.
    pub description: Option<String>,
    /// The higher this value is, the more useful this port is as a default.
    pub priority: u32,
    /// Availability status of this port.
    #[serde(serialize_with = "ser_port_available")]
    pub available: def::PortAvailable,
    /// The direction of this port.
    #[serde(serialize_with = "ser_flag_set")]
    pub direction: direction::FlagSet,
    /// Property list.
    pub proplist: PAProplist,
    /// Latency offset of the port that gets added to the sink/source latency when the port is
    /// active.
    pub latency_offset: i64,
}

impl<'a> From<&'a CardPortInfo<'a>> for PACardPortInfo {
    fn from(value: &'a CardPortInfo<'a>) -> Self {
        PACardPortInfo {
            name: cow!(value.name),
            description: cow!(value.description),
            priority: value.priority,
            available: value.available,
            direction: value.direction,
            proplist: value.proplist.clone().into(),
            latency_offset: value.latency_offset,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PACardInfo {
    /// Index of this card.
    pub index: u32,
    /// Name of this card.
    pub name: Option<String>,
    /// Index of the owning module, or `None`.
    pub owner_module: Option<u32>,
    /// Driver name.
    pub driver: Option<String>,
    /// Property list.
    pub proplist: PAProplist,
    /// Set of ports.
    pub ports: Vec<PACardPortInfo>,
}

impl<'a> From<&'a CardInfo<'a>> for PACardInfo {
    fn from(value: &'a CardInfo<'a>) -> Self {
        PACardInfo {
            index: value.index,
            name: cow!(value.name),
            owner_module: value.owner_module,
            driver: cow!(value.driver),
            proplist: value.proplist.clone().into(),
            ports: value.ports.iter().map(|p| p.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PAModuleInfo {
    /// Index of the module.
    pub index: u32,
    /// Name of the module.
    pub name: Option<String>,
    /// Argument string of the module.
    pub argument: Option<String>,
    /// Usage counter or `None` if invalid.
    pub n_used: Option<u32>,
    /// Property list.
    pub proplist: PAProplist,
}

impl<'a> From<&'a ModuleInfo<'a>> for PAModuleInfo {
    fn from(value: &'a ModuleInfo<'a>) -> Self {
        PAModuleInfo {
            index: value.index,
            name: cow!(value.name),
            argument: cow!(value.argument),
            n_used: value.n_used,
            proplist: value.proplist.clone().into(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PAVolume(pub Volume);

impl PAVolume {
    /// Volume as a percentage; `0.0` is 0%, and `100.0` is 100%
    pub fn percentage(&self) -> f64 {
        (self.0 .0 as f64 / (Volume::NORMAL.0 as f64)) * 100.0
    }

    /// Volume as a linear factor
    pub fn linear(&self) -> f64 {
        VolumeLinear::from(self.0).0
    }

    /// Volume in decibels
    pub fn decibels(&self) -> f64 {
        VolumeDB::from(self.0).0
    }

    /// Volume actual value (`pa_volume_t`)
    pub fn value(&self) -> u32 {
        self.0 .0
    }
}

impl Serialize for PAVolume {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("raw", &self.value())?;
        map.serialize_entry("linear", &self.linear())?;
        map.serialize_entry("decibels", &self.decibels())?;
        map.serialize_entry("percentage", &self.percentage())?;
        map.end()
    }
}

impl From<Volume> for PAVolume {
    fn from(value: Volume) -> Self {
        PAVolume(value)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PAPosition(pub Position);

impl Serialize for PAPosition {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // HACK: use the debug representation to get the names of the channels
        // Not sure if this is better than just casting to an integer - will revisit if it's ever needed
        s.serialize_str(&format!("{:?}", self.0))
    }
}

impl From<Position> for PAPosition {
    fn from(value: Position) -> Self {
        PAPosition(value)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PAFacility(pub Facility);

impl Serialize for PAFacility {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // HACK: use the debug representation to get the names of the facility
        s.serialize_str(&format!("{:?}", self.0).to_lowercase())
    }
}

#[derive(Debug, Clone)]
pub struct PAChannelMap(pub channelmap::Map);

impl Serialize for PAChannelMap {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("channels", &self.0.len())?;
        map.serialize_entry(
            "map",
            &self
                .0
                .get()
                .iter()
                .map(|p| PAPosition::from(*p))
                .collect::<Vec<_>>(),
        )?;
        map.end()
    }
}

impl From<channelmap::Map> for PAChannelMap {
    fn from(value: channelmap::Map) -> Self {
        PAChannelMap(value)
    }
}

#[derive(Debug, Clone)]
pub struct PAProplist(pub Proplist);

impl Serialize for PAProplist {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = s.serialize_map(None)?;
        for key in self.0.iter() {
            // SAFETY: only returns `None` if the key doesn't exist, but we're iterating keys
            // so it must exist
            let value = self.0.get(&key).unwrap();
            // remove trailing null byte
            let value = &value[..value.len() - 1];
            // TODO: handle non-utf8 as escape codes
            let value = String::from_utf8_lossy(value);
            map.serialize_entry(&key, &value)?;
        }
        map.end()
    }
}

impl From<Proplist> for PAProplist {
    fn from(value: Proplist) -> Self {
        PAProplist(value.clone())
    }
}

#[derive(Debug, Clone)]
pub struct PAInfo(pub format::Info);

impl Serialize for PAInfo {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("encoding", &(self.0.get_encoding() as i8))?;
        map.serialize_entry(
            "properties",
            &PAProplist::from(self.0.get_properties().clone()),
        )?;
        map.end()
    }
}

impl From<format::Info> for PAInfo {
    fn from(value: format::Info) -> Self {
        PAInfo(value.clone())
    }
}
