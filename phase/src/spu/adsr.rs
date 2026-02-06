
/// Generates a volume envelope using ADSR:
/// Attack, Decay, Sustain, Release.
#[derive(Default)]
pub struct ADSRGenerator {
    current_state: State,
}

impl ADSRGenerator {
    /// Init the volume into attack.
    pub fn init(&mut self, lo: u16, hi: u16) {
        self.current_state = State::Attack;
    }

    /// Step the envelope and get the new volume.
    pub fn step(&mut self) -> i16 {
        0x7FFF // TODO
    }
}

#[derive(Default)]
enum State {
    #[default]
    Attack,
    Decay,
    Sustain,
    Release,
}