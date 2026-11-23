//! Choosing which MIDI input port to listen to.

/// Which port to connect to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PortChoice {
    /// Whatever port is listed first.
    #[default]
    First,
    /// The first port whose name contains this text (case-insensitive).
    Named(String),
}

pub fn select_port(names: &[String], choice: &PortChoice) -> Option<usize> {
    match choice {
        PortChoice::First => (!names.is_empty()).then_some(0),
        PortChoice::Named(wanted) => {
            let wanted = wanted.to_lowercase();
            names
                .iter()
                .position(|name| name.to_lowercase().contains(&wanted))
        }
    }
}

/// What the port watcher should do after looking at the current port list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortAction {
    /// Open the port at this index.
    Connect(usize),
    /// The connected port is still there.
    Stay,
    /// The connected port has gone (unplugged): close it.
    Disconnect,
    /// Nothing connected and nothing suitable yet: look again later.
    Wait,
}

/// Decide what to do given the port we're on (if any) and the ports listed
/// now, so keyboards can be plugged in or out while the app runs.
pub fn next_action(connected: Option<&str>, names: &[String], choice: &PortChoice) -> PortAction {
    match connected {
        Some(current) if names.iter().any(|n| n == current) => PortAction::Stay,
        Some(_) => PortAction::Disconnect,
        None => select_port(names, choice).map_or(PortAction::Wait, PortAction::Connect),
    }
}

/// Names of the MIDI input ports currently available.
pub fn list_ports() -> Vec<String> {
    midir::MidiInput::new("keybard-list").map_or_else(
        |_| Vec::new(),
        |input| {
            input
                .ports()
                .iter()
                .filter_map(|p| input.port_name(p).ok())
                .collect()
        },
    )
}
