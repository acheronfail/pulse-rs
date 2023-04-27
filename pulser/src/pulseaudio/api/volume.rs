use std::error::Error;
use std::str::FromStr;

use libpulse_binding::channelmap::Position;
use libpulse_binding::volume::{Volume, VolumeDB, VolumeLinear};
use serde::Serialize;

use super::{PAPosition, PAVolume};

/// Used when requesting the volume from an object
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub struct VolumeReading {
    /// Which channel this volume belongs to
    pub channel: PAPosition,
    pub(crate) volume: PAVolume,
}

impl VolumeReading {
    pub fn new(channel: &Position, volume: &Volume) -> VolumeReading {
        VolumeReading {
            channel: PAPosition(*channel),
            volume: PAVolume(*volume),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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

impl FromStr for PAVol {
    type Err = Box<dyn Error>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_string();

        // "<FLOAT>L" (linear)
        if s.ends_with("L") {
            s.pop();
            return Ok(PAVol::Linear(s.trim().parse::<f64>()?));
        }

        // "<FLOAT>dB" (decibels)
        if s.ends_with("dB") {
            s.pop();
            s.pop();
            return Ok(PAVol::Decibels(s.trim().parse::<f64>()?));
        }

        // "<INT|FLOAT>%" (percentage)
        if s.ends_with("%") {
            s.pop();
            return Ok(match s.trim().parse::<f64>() {
                Ok(f) => PAVol::Percentage(f),
                Err(_) => PAVol::Percentage(s.trim().parse::<u32>()? as f64),
            });
        }

        // "<INT>" (integer)
        Ok(PAVol::Value(s.trim().parse::<u32>()?))
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
