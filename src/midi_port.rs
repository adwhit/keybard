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
