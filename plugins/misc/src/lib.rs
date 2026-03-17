mod commands;

mod ext;
pub use ext::*;

const PLUGIN_NAME: &str = "misc";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::get_git_hash::<tauri::Wry>,
            commands::get_fingerprint::<tauri::Wry>,
            commands::get_device_info::<tauri::Wry>,
            commands::opinionated_md_to_html::<tauri::Wry>,
            commands::parse_meeting_link::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        builder
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[test]
    fn test_parse_meeting_link() {
        let app = create_app(tauri::test::mock_builder());

        struct TestCase {
            name: &'static str,
            input: &'static str,
            expected: &'static str,
        }

        let test_cases = vec![
            TestCase {
                name: "cal.com link",
                input: indoc::indoc! {r#"
                    What:
                    30 Min Meeting between Alice Smith and Bob Johnson
                    Invitee Time Zone:
                    Asia/Seoul
                    Who:
                    Alice Smith - Organizer
                    alice.smith@example.com
                    Bob Johnson
                    bob.johnson@example.com
                    Where:
                    https://app.cal.com/video/d713v9w1d2krBptPtwUAnJ
                    Need to reschedule or cancel? https://cal.com/booking/d713v9w1d2krBptPtwUAnJ?changes=true
                "#},
                expected: "https://app.cal.com/video/d713v9w1d2krBptPtwUAnJ",
            },
            TestCase {
                name: "zoom link",
                input: indoc::indoc! {r#"
                    What:
                    Let's chat! between Alice Smith and Charlie Davis
                    Invitee Time Zone:
                    Asia/Seoul
                    Who:
                    Alice Smith - Organizer
                    alice.smith@example.com
                    Charlie Davis
                    charlie.davis@example.com
                    Where:
                    https://us05web.zoom.us/j/87636383039?pwd=NOWbxkY9GNblR0yaLKaIzcy76IWRoj.1
                    Description
                    ****Hey, I'm Alice, co-founder of Hyprnote.****
                    Thanks for taking the time to visit this page—excited that you're booking a call with me.
                    Looking forward to chatting soon!
                    Best,
                    Alice
                    Need to reschedule or cancel? https://cal.com/booking/dnHcnBV5RX8Jp3iq2E2QTe?changes=true
                "#},
                expected: "https://us05web.zoom.us/j/87636383039?pwd=NOWbxkY9GNblR0yaLKaIzcy76IWRoj.1",
            },
            TestCase {
                name: "google meet link",
                input: indoc::indoc! {r#"
                    https://meet.google.com/xhv-ubut-zph
                    tel:+1%20650-817-8427;205595809%23
                    전화번호 더보기: https://tel.meet/xhv-ubut-zph?pin=4030200140074&hs=7
                    https://support.google.com/a/users/answer/9282720에서 Meet에 대해 자세히 알아보세요.
                    이 섹션을 수정하지 마시기 바랍니다.
                "#},
                expected: "https://meet.google.com/xhv-ubut-zph",
            },
            TestCase {
                name: "zoom meeting with html formatting",
                input: indoc::indoc! {r#"
                    <p>──────────<br/>John Jeong is inviting you to a scheduled Zoom meeting.<br/>Join Zoom Meeting<br/>https://hyprnote.zoom.us/j/86746313244?pwd=zFIICnVHzPim44QcYGbLCAAqtBrGzx.1<br/><br/>
                    View meeting insights with Zoom AI Companion<br/>https://hyprnote.zoom.us/launch/edl?muid=8fff7a40-04e0-4a8e-ae46-026a86793906<br/><br/>
                    Meeting ID: 867 4631 3244<br/>Passcode: 291681</p>
                "#},
                expected: "https://hyprnote.zoom.us/j/86746313244?pwd=zFIICnVHzPim44QcYGbLCAAqtBrGzx.1",
            },
            TestCase {
                name: "korean google meet link",
                input: indoc::indoc! {r#"
                    -::~:~::~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~::~:~::-
                    Google Meet으로 참석: https://meet.google.com/xkf-xcmo-rwh
                    또는 다음 전화번호로 전화 걸기: (US) +1 402-732-7278 PIN: 765104423#
                    전화번호 더보기: https://tel.meet/xkf-xcmo-rwh?pin=5171333427182&hs=7

                    https://support.google.com/a/users/answer/9282720에서 Meet에 대해 자세히 알아보세요.

                    이 섹션을 수정하지 마시기 바랍니다.
                    -::~:~::~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~::~:~::-
                "#},
                expected: "https://meet.google.com/xkf-xcmo-rwh",
            },
            TestCase {
                name: "zoom meeting with dial-in details",
                input: indoc::indoc! {r#"
                    Join Zoom Meeting
                    https://hyprnote.zoom.us/j/86746313244?pwd=zFIICnVHzPim44QcYGbLCAAqtBrGzx.1

                    Meeting ID: 867 4631 3244
                    Passcode: 291681

                    ---

                    One tap mobile
                    +16694449171,,86746313244#,,,,*291681# US
                    +16699006833,,86746313244#,,,,*291681# US (San Jose)

                    ---

                    Dial by your location
                    • +1 669 444 9171 US
                    • +1 669 900 6833 US (San Jose)
                    • +1 253 205 0468 US
                    • +1 253 215 8782 US (Tacoma)
                    • +1 346 248 7799 US (Houston)
                    • +1 719 359 4580 US
                    • +1 312 626 6799 US (Chicago)
                    • +1 360 209 5623 US
                    • +1 386 347 5053 US
                    • +1 507 473 4847 US
                    • +1 564 217 2000 US
                    • +1 646 931 3860 US
                    • +1 689 278 1000 US
                    • +1 929 205 6099 US (New York)
                    • +1 301 715 8592 US (Washington DC)
                    • +1 305 224 1968 US
                    • +1 309 205 3325 US

                    Meeting ID: 867 4631 3244
                    Passcode: 291681

                    Find your local number: https://hyprnote.zoom.us/u/kdoIeyBH9b

                    ---

                    Join by SIP
                    • 86746313244@zoomcrc.com
                "#},
                expected: "https://hyprnote.zoom.us/j/86746313244?pwd=zFIICnVHzPim44QcYGbLCAAqtBrGzx.1",
            },
        ];

        for test_case in test_cases {
            let result = app.misc().parse_meeting_link(test_case.input);
            assert_eq!(
                result,
                Some(test_case.expected.to_string()),
                "Failed test case: {}",
                test_case.name
            );
        }
    }
}
