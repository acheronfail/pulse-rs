use libpulse_binding::channelmap::Position;
use libpulse_binding::volume::{Volume, VolumeDB, VolumeLinear};

/// Used when requesting the volume from an object
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VolumeReading {
    /// Which channel this volume belongs to
    pub channel: Position,
    pub(crate) volume: Volume,
}

impl VolumeReading {
    pub fn new(channel: &Position, volume: &Volume) -> VolumeReading {
        VolumeReading {
            channel: *channel,
            volume: *volume,
        }
    }

    /// Volume as a percentage; `0.0` is 0%, and `100.0` is 100%
    pub fn percentage(&self) -> f64 {
        (self.volume.0 as f64 / (Volume::NORMAL.0 as f64)) * 100.0
    }

    /// Volume as a linear factor
    pub fn linear(&self) -> f64 {
        VolumeLinear::from(self.volume).0
    }

    /// Volume in decibels
    pub fn decibels(&self) -> f64 {
        VolumeDB::from(self.volume).0
    }

    /// Volume actual value (`pa_volume_t`)
    pub fn value(&self) -> u32 {
        self.volume.0
    }
}

#[derive(Debug, Clone)]
pub struct VolumeReadings {
    pub(crate) inner: Vec<VolumeReading>,
}

impl From<Vec<VolumeReading>> for VolumeReadings {
    fn from(value: Vec<VolumeReading>) -> Self {
        VolumeReadings { inner: value }
    }
}

impl FromIterator<VolumeReading> for VolumeReadings {
    fn from_iter<T: IntoIterator<Item = VolumeReading>>(iter: T) -> Self {
        let inner = iter.into_iter().collect::<Vec<VolumeReading>>();
        VolumeReadings { inner }
    }
}

/// Abstraction used to represent a volume
#[derive(Debug, Copy, Clone)]
pub enum PAVol {
    /// Volume as a percentage; `0.0` is 0%, and `100.0` is 100%
    Percentage(f64),
    Decibels(f64),
    Linear(f64),
    Value(u32),
}

impl PAVol {
    pub fn value(&self) -> u32 {
        let v: Volume = (*self).into();
        v.0
    }
}

impl From<PAVol> for Volume {
    fn from(value: PAVol) -> Self {
        match value {
            PAVol::Value(value) => Volume(value),
            PAVol::Decibels(db) => VolumeDB(db).into(),
            PAVol::Linear(lin) => VolumeLinear(lin).into(),
            // libpulse doesn't seem to offer a way to calculate percentages...
            PAVol::Percentage(pct) => Volume((Volume::NORMAL.0 as f64 * (pct / 100.0)) as u32),
        }
    }
}

/// Used to set the volume of a pulseaudio object
#[derive(Debug, Clone)]
pub enum VolumeSpec {
    /// Single volume; this will set each channel to this volume
    All(PAVol),
    /// List of volumes; each is a tuple of `Position` (channel) and `PAVol` (volume for that channel)
    /// Length of this `Vec` cannot exceed `libpulse_binding::sample::Spec::CHANNELS_MAX`
    Channels(Vec<PAVol>),
}
