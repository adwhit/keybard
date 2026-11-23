use keybard::midi_port::{PortChoice, select_port};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn first_port_by_default() {
    assert_eq!(
        select_port(&names(&["IAC Bus", "Piano"]), &PortChoice::First),
        Some(0)
    );
    assert_eq!(select_port(&[], &PortChoice::First), None);
}

#[test]
fn named_ports_match_case_insensitive_substrings() {
    let ports = names(&["IAC Driver Bus 1", "Yamaha P-125 Port 1", "Arturia KeyStep"]);
    let pick = |s: &str| select_port(&ports, &PortChoice::Named(s.to_owned()));
    assert_eq!(pick("yamaha"), Some(1));
    assert_eq!(pick("KEYSTEP"), Some(2));
    assert_eq!(pick("roland"), None);
}
