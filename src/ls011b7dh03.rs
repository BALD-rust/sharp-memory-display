use hal::spi::{Mode, Phase, Polarity};

pub(crate) const MODE: Mode = Mode {
    polarity: Polarity::IdleLow,
    phase: Phase::CaptureOnFirstTransition,
};
pub(crate) const WIDTH: usize = 160;
pub(crate) const HEIGHT: usize = 68;