//! Simulation speed control, matching the rates offered by Z80 Simulator IDE.

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Rate {
    /// One instruction per command from the user; registers are editable.
    StepByStep,
    Slow,
    #[default]
    Normal,
    Fast,
    /// As fast as the host allows, still refreshing the display every frame.
    ExtremelyFast,
    /// As fast as the host allows, refreshing only occasionally so that the
    /// display costs almost nothing.
    Ultimate,
}

impl Rate {
    /// How long to wait between instructions, or `None` to run flat out.
    pub fn delay(self) -> Option<Duration> {
        let millis = match self {
            Rate::StepByStep => return None,
            Rate::Slow => 1500,
            Rate::Normal => 250,
            Rate::Fast => 50,
            Rate::ExtremelyFast | Rate::Ultimate => return None,
        };
        Some(Duration::from_millis(millis))
    }

    /// True when the simulation only advances on an explicit step command.
    pub fn is_manual(self) -> bool {
        matches!(self, Rate::StepByStep)
    }

    /// True when the UI should be refreshed after every instruction rather
    /// than on a timer.
    pub fn refreshes_every_instruction(self) -> bool {
        !matches!(self, Rate::ExtremelyFast | Rate::Ultimate)
    }

    /// How often to push state to the display while free-running.
    pub fn refresh_interval(self, ultimate: Duration) -> Duration {
        match self {
            Rate::Ultimate => ultimate,
            // Roughly one frame; fast enough to look live, cheap enough not
            // to dominate the simulation.
            _ => Duration::from_millis(16),
        }
    }
}
