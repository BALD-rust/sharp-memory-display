use hal::spi::{Mode, Phase, Polarity};

pub(crate) const MODE: Mode = Mode {
    polarity: Polarity::IdleLow,
    phase: Phase::CaptureOnSecondTransition,
};
pub(crate) const WIDTH: usize = 240;
pub(crate) const HEIGHT: usize = 240;