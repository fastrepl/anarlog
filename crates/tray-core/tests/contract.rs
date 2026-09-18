use tray_core::{
    TrayAgendaEvent, TrayAgendaSection, agenda_sections, contract, menu_bar_title,
    next_schedule_refresh_ms,
};

#[test]
fn tray_fixture_is_the_shared_schedule_contract() {
    for case in contract::cases() {
        let actual_agenda = agenda_sections(&case.events, case.now_ms, case.show_events);
        let expected_agenda: Vec<TrayAgendaSection> = case
            .expect
            .agenda
            .iter()
            .map(|section| TrayAgendaSection {
                label: section.label.clone(),
                events: section
                    .events
                    .iter()
                    .map(|event| TrayAgendaEvent {
                        id: event.id.clone(),
                        label: event.label.clone(),
                    })
                    .collect(),
            })
            .collect();
        assert_eq!(actual_agenda, expected_agenda, "{} agenda", case.name);
        assert_eq!(
            menu_bar_title(
                &case.events,
                case.now_ms,
                case.show_events,
                case.is_recording,
                case.recording_title.as_deref()
            ),
            case.expect.title,
            "{} title",
            case.name
        );
        assert_eq!(
            next_schedule_refresh_ms(
                &case.events,
                case.now_ms,
                case.show_events,
                case.is_recording
            ),
            case.expect.refresh_ms,
            "{} refresh",
            case.name
        );
    }
}
