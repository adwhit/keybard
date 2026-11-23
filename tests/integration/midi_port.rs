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

mod watching {
    use keybard::midi_port::{PortAction, PortChoice, next_action};

    use super::names;

    #[test]
    fn connects_when_a_keyboard_appears() {
        assert_eq!(next_action(None, &[], &PortChoice::First), PortAction::Wait);
        assert_eq!(
            next_action(None, &names(&["Piano"]), &PortChoice::First),
            PortAction::Connect(0)
        );
    }

    #[test]
    fn stays_on_a_port_while_it_is_listed() {
        let ports = names(&["IAC Bus", "Piano"]);
        assert_eq!(
            next_action(Some("Piano"), &ports, &PortChoice::First),
            PortAction::Stay
        );
    }

    #[test]
    fn lets_go_when_the_keyboard_is_unplugged() {
        assert_eq!(
            next_action(Some("Piano"), &names(&["IAC Bus"]), &PortChoice::First),
            PortAction::Disconnect
        );
    }

    #[test]
    fn named_ports_wait_for_a_match() {
        let choice = PortChoice::Named("yamaha".into());
        assert_eq!(
            next_action(None, &names(&["IAC Bus"]), &choice),
            PortAction::Wait
        );
        assert_eq!(
            next_action(None, &names(&["IAC Bus", "Yamaha P-125"]), &choice),
            PortAction::Connect(1)
        );
    }
}
