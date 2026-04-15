//! [`WeirollPlanner`] — builder for Weiroll scripts.
//!
//! Accumulates [`WeirollCommand`]s and state slots, then finalises them
//! into an executable [`WeirollScript`] via [`WeirollPlanner::plan`].

use super::types::{WeirollCommand, WeirollScript};

/// Builder that accumulates commands and state slots into a Weiroll script.
///
/// # Example
///
/// ```
/// use alloy_primitives::Address;
/// use cow_weiroll::{WeirollCommand, WeirollPlanner};
///
/// let mut planner = WeirollPlanner::new();
/// planner.add_command(WeirollCommand {
///     flags: 0,
///     value: 0,
///     gas: 0,
///     target: Address::ZERO,
///     selector: [0; 4],
///     in_out: [0; 4],
/// });
/// let script = planner.plan();
/// assert_eq!(script.command_count(), 1);
/// ```
#[derive(Debug, Default)]
pub struct WeirollPlanner {
    commands: Vec<WeirollCommand>,
    state: Vec<Vec<u8>>,
}

impl WeirollPlanner {
    /// Create an empty [`WeirollPlanner`] with no commands or state slots.
    ///
    /// # Returns
    ///
    /// A new planner ready to receive commands via [`add_command`](Self::add_command).
    #[must_use]
    pub const fn new() -> Self {
        Self { commands: vec![], state: vec![] }
    }

    /// Append a raw [`WeirollCommand`] to the script.
    ///
    /// Commands are executed in the order they are added.
    ///
    /// # Parameters
    ///
    /// * `cmd` — the command to append.
    ///
    /// # Returns
    ///
    /// `&mut Self` for method chaining.
    pub fn add_command(&mut self, cmd: WeirollCommand) -> &mut Self {
        self.commands.push(cmd);
        self
    }

    /// Append a state slot (ABI-encoded value) and return its zero-based
    /// index.
    ///
    /// State slots hold the arguments and return values that flow between
    /// commands during Weiroll execution.
    ///
    /// # Parameters
    ///
    /// * `data` — the ABI-encoded bytes for this slot.
    ///
    /// # Returns
    ///
    /// The zero-based index of the newly added slot (use this index in
    /// [`WeirollCommand::in_out`] to wire commands together).
    pub fn add_state_slot(&mut self, data: Vec<u8>) -> usize {
        let idx = self.state.len();
        self.state.push(data);
        idx
    }

    /// Number of commands added so far.
    ///
    /// # Returns
    ///
    /// The count of commands in this planner.
    #[must_use]
    pub const fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Number of state slots added so far.
    ///
    /// # Returns
    ///
    /// The count of state slots in this planner.
    #[must_use]
    pub const fn state_slot_count(&self) -> usize {
        self.state.len()
    }

    /// Finalise the planner and produce an executable [`WeirollScript`].
    ///
    /// Each command is packed into a 32-byte word via
    /// [`WeirollCommand::pack`]. The planner is consumed.
    ///
    /// # Returns
    ///
    /// A [`WeirollScript`] containing the packed commands and state slots.
    #[must_use]
    pub fn plan(self) -> WeirollScript {
        WeirollScript {
            commands: self.commands.iter().map(WeirollCommand::pack).collect(),
            state: self.state,
        }
    }
}
