#![allow(unreachable_patterns)]

#[cfg(target_os = "macos")]
pub fn is_builtin_display_inactive() -> bool {
    use objc2_core_graphics::{CGDisplayIsBuiltin, CGGetOnlineDisplayList};

    let mut display_count: u32 = 0;
    let mut displays: [u32; 16] = [0; 16];

    unsafe {
        let result = CGGetOnlineDisplayList(16, displays.as_mut_ptr(), &mut display_count);
        if result.0 != 0 {
            return false;
        }
    }

    for display in displays.iter().take(display_count as usize) {
        if CGDisplayIsBuiltin(*display) {
            return false;
        }
    }

    display_count > 0
}

#[cfg(not(target_os = "macos"))]
pub fn is_builtin_display_inactive() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn hw_model() -> std::io::Result<String> {
    use libc::{c_void, size_t};
    use std::ffi::CString;

    unsafe {
        let name = CString::new("hw.model").unwrap();

        let mut len: size_t = 0;
        if libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(std::io::Error::last_os_error());
        }

        let mut buf = vec![0u8; len];
        if libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(std::io::Error::last_os_error());
        }

        if let Some(pos) = buf.iter().position(|&b| b == 0) {
            buf.truncate(pos);
        }

        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hw_model() -> std::io::Result<String> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "hw_model is only supported on macOS",
    ))
}

#[derive(strum::Display)]
pub enum Family {
    #[strum(to_string = "Mac mini")]
    MacMini,
    #[strum(to_string = "Mac Studio")]
    MacStudio,
    #[strum(to_string = "MacBook Pro")]
    MacBookPro,
    MacBook,
    #[strum(to_string = "MacBook Air")]
    MacBookAir,
    #[strum(to_string = "iMac")]
    IMac,
    #[strum(to_string = "iMac Pro")]
    IMacPro,
    #[strum(to_string = "Mac Pro")]
    MacPro,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, strum::EnumString, strum::Display, strum::AsRefStr)]
pub enum ModelIdentifier {
    // Mac mini: https://support.apple.com/en-us/102852
    #[strum(serialize = "Mac16,11", to_string = "Mac mini (2024)")]
    Mac16_11,
    #[strum(serialize = "Mac16,10", to_string = "Mac mini (2024)")]
    Mac16_10,
    #[strum(serialize = "Mac14,3", to_string = "Mac mini (2023)")]
    Mac14_3,
    #[strum(serialize = "Mac14,12", to_string = "Mac mini (2023)")]
    Mac14_12,
    #[strum(serialize = "Macmini9,1", to_string = "Mac mini (M1, 2020)")]
    Macmini9_1,
    #[strum(serialize = "Macmini8,1", to_string = "Mac mini (2018)")]
    Macmini8_1,
    #[strum(serialize = "Macmini7,1", to_string = "Mac mini (Late 2014)")]
    Macmini7_1,
    #[strum(serialize = "Macmini6,1", to_string = "Mac mini (Late 2012)")]
    Macmini6_1,
    #[strum(serialize = "Macmini6,2", to_string = "Mac mini (Late 2012)")]
    Macmini6_2,
    #[strum(serialize = "Macmini5,1", to_string = "Mac mini (Mid 2011)")]
    Macmini5_1,
    #[strum(serialize = "Macmini5,2", to_string = "Mac mini (Mid 2011)")]
    Macmini5_2,
    #[strum(serialize = "Macmini4,1", to_string = "Mac mini (Mid 2010)")]
    Macmini4_1,
    #[strum(serialize = "Macmini3,1", to_string = "Mac mini (2009)")]
    Macmini3_1,

    // Mac Studio: https://support.apple.com/en-us/102231
    #[strum(serialize = "Mac16,9", to_string = "Mac Studio (2025)")]
    Mac16_9,
    #[strum(serialize = "Mac15,14", to_string = "Mac Studio (2025)")]
    Mac15_14,
    #[strum(serialize = "Mac14,13", to_string = "Mac Studio (2023)")]
    Mac14_13,
    #[strum(serialize = "Mac14,14", to_string = "Mac Studio (2023)")]
    Mac14_14,
    #[strum(serialize = "Mac13,1", to_string = "Mac Studio (2022)")]
    Mac13_1,
    #[strum(serialize = "Mac13,2", to_string = "Mac Studio (2022)")]
    Mac13_2,

    // MacBook Pro: https://support.apple.com/en-us/108052
    #[strum(serialize = "Mac17,2", to_string = "MacBook Pro (14-inch, M5)")]
    Mac17_2,
    #[strum(serialize = "Mac16,1", to_string = "MacBook Pro (14-inch, 2024)")]
    Mac16_1,
    #[strum(serialize = "Mac16,6", to_string = "MacBook Pro (14-inch, 2024)")]
    Mac16_6,
    #[strum(serialize = "Mac16,8", to_string = "MacBook Pro (14-inch, 2024)")]
    Mac16_8,
    #[strum(serialize = "Mac16,7", to_string = "MacBook Pro (16-inch, 2024)")]
    Mac16_7,
    #[strum(serialize = "Mac16,5", to_string = "MacBook Pro (16-inch, 2024)")]
    Mac16_5,
    #[strum(serialize = "Mac15,3", to_string = "MacBook Pro (14-inch, Nov 2023)")]
    Mac15_3,
    #[strum(serialize = "Mac15,6", to_string = "MacBook Pro (14-inch, Nov 2023)")]
    Mac15_6,
    #[strum(serialize = "Mac15,8", to_string = "MacBook Pro (14-inch, Nov 2023)")]
    Mac15_8,
    #[strum(serialize = "Mac15,10", to_string = "MacBook Pro (14-inch, Nov 2023)")]
    Mac15_10,
    #[strum(serialize = "Mac15,7", to_string = "MacBook Pro (16-inch, Nov 2023)")]
    Mac15_7,
    #[strum(serialize = "Mac15,9", to_string = "MacBook Pro (16-inch, Nov 2023)")]
    Mac15_9,
    #[strum(serialize = "Mac15,11", to_string = "MacBook Pro (16-inch, Nov 2023)")]
    Mac15_11,
    #[strum(serialize = "Mac14,5", to_string = "MacBook Pro (14-inch, 2023)")]
    Mac14_5,
    #[strum(serialize = "Mac14,9", to_string = "MacBook Pro (14-inch, 2023)")]
    Mac14_9,
    #[strum(serialize = "Mac14,6", to_string = "MacBook Pro (16-inch, 2023)")]
    Mac14_6,
    #[strum(serialize = "Mac14,10", to_string = "MacBook Pro (16-inch, 2023)")]
    Mac14_10,
    #[strum(serialize = "Mac14,7", to_string = "MacBook Pro (13-inch, M2, 2022)")]
    Mac14_7,
    #[strum(
        serialize = "MacBookPro18,3",
        to_string = "MacBook Pro (14-inch, 2021)"
    )]
    MacBookPro18_3,
    #[strum(
        serialize = "MacBookPro18,4",
        to_string = "MacBook Pro (14-inch, 2021)"
    )]
    MacBookPro18_4,
    #[strum(
        serialize = "MacBookPro18,1",
        to_string = "MacBook Pro (16-inch, 2021)"
    )]
    MacBookPro18_1,
    #[strum(
        serialize = "MacBookPro18,2",
        to_string = "MacBook Pro (16-inch, 2021)"
    )]
    MacBookPro18_2,
    #[strum(
        serialize = "MacBookPro17,1",
        to_string = "MacBook Pro (13-inch, M1, 2020)"
    )]
    MacBookPro17_1,
    #[strum(
        serialize = "MacBookPro16,3",
        to_string = "MacBook Pro (13-inch, 2020, Two Thunderbolt 3 ports)"
    )]
    MacBookPro16_3,
    #[strum(
        serialize = "MacBookPro16,2",
        to_string = "MacBook Pro (13-inch, 2020, Four Thunderbolt 3 ports)"
    )]
    MacBookPro16_2,
    #[strum(
        serialize = "MacBookPro16,1",
        to_string = "MacBook Pro (16-inch, 2019)"
    )]
    MacBookPro16_1,
    #[strum(
        serialize = "MacBookPro16,4",
        to_string = "MacBook Pro (16-inch, 2019)"
    )]
    MacBookPro16_4,
    #[strum(
        serialize = "MacBookPro15,4",
        to_string = "MacBook Pro (13-inch, 2019, Two Thunderbolt 3 ports)"
    )]
    MacBookPro15_4,
    #[strum(
        serialize = "MacBookPro15,1",
        to_string = "MacBook Pro (15-inch, 2019)"
    )]
    MacBookPro15_1,
    #[strum(
        serialize = "MacBookPro15,3",
        to_string = "MacBook Pro (15-inch, 2019)"
    )]
    MacBookPro15_3,
    #[strum(
        serialize = "MacBookPro15,2",
        to_string = "MacBook Pro (13-inch, 2019, Four Thunderbolt 3 ports)"
    )]
    MacBookPro15_2,
    #[strum(
        serialize = "MacBookPro14,3",
        to_string = "MacBook Pro (15-inch, 2017)"
    )]
    MacBookPro14_3,
    #[strum(
        serialize = "MacBookPro14,2",
        to_string = "MacBook Pro (13-inch, 2017, Four Thunderbolt 3 ports)"
    )]
    MacBookPro14_2,
    #[strum(
        serialize = "MacBookPro14,1",
        to_string = "MacBook Pro (13-inch, 2017, Two Thunderbolt 3 ports)"
    )]
    MacBookPro14_1,
    #[strum(
        serialize = "MacBookPro13,3",
        to_string = "MacBook Pro (15-inch, 2016)"
    )]
    MacBookPro13_3,
    #[strum(
        serialize = "MacBookPro13,2",
        to_string = "MacBook Pro (13-inch, 2016, Four Thunderbolt 3 ports)"
    )]
    MacBookPro13_2,
    #[strum(
        serialize = "MacBookPro13,1",
        to_string = "MacBook Pro (13-inch, 2016, Two Thunderbolt 3 ports)"
    )]
    MacBookPro13_1,
    #[strum(
        serialize = "MacBookPro11,4",
        to_string = "MacBook Pro (Retina, 15-inch, Mid 2015)"
    )]
    MacBookPro11_4,
    #[strum(
        serialize = "MacBookPro11,5",
        to_string = "MacBook Pro (Retina, 15-inch, Mid 2015)"
    )]
    MacBookPro11_5,
    #[strum(
        serialize = "MacBookPro12,1",
        to_string = "MacBook Pro (Retina, 13-inch, Early 2015)"
    )]
    MacBookPro12_1,
    #[strum(
        serialize = "MacBookPro11,2",
        to_string = "MacBook Pro (Retina, 15-inch, Mid 2014)"
    )]
    MacBookPro11_2,
    #[strum(
        serialize = "MacBookPro11,3",
        to_string = "MacBook Pro (Retina, 15-inch, Mid 2014)"
    )]
    MacBookPro11_3,
    #[strum(
        serialize = "MacBookPro11,1",
        to_string = "MacBook Pro (Retina, 13-inch, Mid 2014)"
    )]
    MacBookPro11_1,
    #[strum(
        serialize = "MacBookPro10,1",
        to_string = "MacBook Pro (Retina, 15-inch, Early 2013)"
    )]
    MacBookPro10_1,
    #[strum(
        serialize = "MacBookPro10,2",
        to_string = "MacBook Pro (Retina, 13-inch, Early 2013)"
    )]
    MacBookPro10_2,
    #[strum(
        serialize = "MacBookPro9,1",
        to_string = "MacBook Pro (15-inch, Mid 2012)"
    )]
    MacBookPro9_1,
    #[strum(
        serialize = "MacBookPro9,2",
        to_string = "MacBook Pro (13-inch, Mid 2012)"
    )]
    MacBookPro9_2,
    #[strum(serialize = "MacBookPro8,3", to_string = "MacBook Pro (17-inch, 2011)")]
    MacBookPro8_3,
    #[strum(serialize = "MacBookPro8,2", to_string = "MacBook Pro (15-inch, 2011)")]
    MacBookPro8_2,
    #[strum(serialize = "MacBookPro8,1", to_string = "MacBook Pro (13-inch, 2011)")]
    MacBookPro8_1,
    #[strum(
        serialize = "MacBookPro6,1",
        to_string = "MacBook Pro (17-inch, Mid 2010)"
    )]
    MacBookPro6_1,
    #[strum(
        serialize = "MacBookPro6,2",
        to_string = "MacBook Pro (15-inch, Mid 2010)"
    )]
    MacBookPro6_2,
    #[strum(
        serialize = "MacBookPro7,1",
        to_string = "MacBook Pro (13-inch, Mid 2010)"
    )]
    MacBookPro7_1,
    #[strum(serialize = "MacBookPro5,2", to_string = "MacBook Pro (17-inch, 2009)")]
    MacBookPro5_2,
    #[strum(
        serialize = "MacBookPro5,3",
        to_string = "MacBook Pro (15-inch, Mid 2009)"
    )]
    MacBookPro5_3,
    #[strum(
        serialize = "MacBookPro5,5",
        to_string = "MacBook Pro (13-inch, Mid 2009)"
    )]
    MacBookPro5_5,
    #[strum(
        serialize = "MacBookPro5,1",
        to_string = "MacBook Pro (15-inch, Late 2008)"
    )]
    MacBookPro5_1,
    #[strum(serialize = "MacBookPro4,1", to_string = "MacBook Pro (Early 2008)")]
    MacBookPro4_1,

    // MacBook: https://support.apple.com/en-us/103257
    #[strum(
        serialize = "MacBook10,1",
        to_string = "MacBook (Retina, 12-inch, 2017)"
    )]
    MacBook10_1,
    #[strum(
        serialize = "MacBook9,1",
        to_string = "MacBook (Retina, 12-inch, Early 2016)"
    )]
    MacBook9_1,
    #[strum(
        serialize = "MacBook8,1",
        to_string = "MacBook (Retina, 12-inch, Early 2015)"
    )]
    MacBook8_1,
    #[strum(serialize = "MacBook7,1", to_string = "MacBook (13-inch, Mid 2010)")]
    MacBook7_1,
    #[strum(serialize = "MacBook6,1", to_string = "MacBook (13-inch, Late 2009)")]
    MacBook6_1,
    #[strum(serialize = "MacBook5,2", to_string = "MacBook (13-inch, 2009)")]
    MacBook5_2,

    // MacBook Air: https://support.apple.com/en-us/102869
    #[strum(serialize = "Mac16,13", to_string = "MacBook Air (15-inch, M4, 2025)")]
    Mac16_13,
    #[strum(serialize = "Mac16,12", to_string = "MacBook Air (13-inch, M4, 2025)")]
    Mac16_12,
    #[strum(serialize = "Mac15,13", to_string = "MacBook Air (15-inch, M3, 2024)")]
    Mac15_13,
    #[strum(serialize = "Mac15,12", to_string = "MacBook Air (13-inch, M3, 2024)")]
    Mac15_12,
    #[strum(serialize = "Mac14,15", to_string = "MacBook Air (15-inch, M2, 2023)")]
    Mac14_15,
    #[strum(serialize = "Mac14,2", to_string = "MacBook Air (M2, 2022)")]
    Mac14_2,
    #[strum(serialize = "MacBookAir10,1", to_string = "MacBook Air (M1, 2020)")]
    MacBookAir10_1,
    #[strum(
        serialize = "MacBookAir9,1",
        to_string = "MacBook Air (Retina, 13-inch, 2020)"
    )]
    MacBookAir9_1,
    #[strum(
        serialize = "MacBookAir8,2",
        to_string = "MacBook Air (Retina, 13-inch, 2019)"
    )]
    MacBookAir8_2,
    #[strum(
        serialize = "MacBookAir8,1",
        to_string = "MacBook Air (Retina, 13-inch, 2018)"
    )]
    MacBookAir8_1,
    #[strum(serialize = "MacBookAir7,2", to_string = "MacBook Air (13-inch, 2017)")]
    MacBookAir7_2,
    #[strum(
        serialize = "MacBookAir7,1",
        to_string = "MacBook Air (11-inch, Early 2015)"
    )]
    MacBookAir7_1,
    #[strum(
        serialize = "MacBookAir6,2",
        to_string = "MacBook Air (13-inch, Mid 2013)"
    )]
    MacBookAir6_2,
    #[strum(
        serialize = "MacBookAir6,1",
        to_string = "MacBook Air (11-inch, Mid 2013)"
    )]
    MacBookAir6_1,
    #[strum(
        serialize = "MacBookAir5,2",
        to_string = "MacBook Air (13-inch, Mid 2012)"
    )]
    MacBookAir5_2,
    #[strum(
        serialize = "MacBookAir5,1",
        to_string = "MacBook Air (11-inch, Mid 2012)"
    )]
    MacBookAir5_1,
    #[strum(
        serialize = "MacBookAir4,2",
        to_string = "MacBook Air (13-inch, Mid 2011)"
    )]
    MacBookAir4_2,
    #[strum(
        serialize = "MacBookAir4,1",
        to_string = "MacBook Air (11-inch, Mid 2011)"
    )]
    MacBookAir4_1,
    #[strum(
        serialize = "MacBookAir3,2",
        to_string = "MacBook Air (13-inch, Late 2010)"
    )]
    MacBookAir3_2,
    #[strum(
        serialize = "MacBookAir3,1",
        to_string = "MacBook Air (11-inch, Late 2010)"
    )]
    MacBookAir3_1,
    #[strum(serialize = "MacBookAir2,1", to_string = "MacBook Air (Mid 2009)")]
    MacBookAir2_1,

    // iMac: https://support.apple.com/en-us/108054
    #[strum(serialize = "Mac16,3", to_string = "iMac (24-inch, 2024, Four ports)")]
    Mac16_3,
    #[strum(serialize = "Mac16,2", to_string = "iMac (24-inch, 2024, Two ports)")]
    Mac16_2,
    #[strum(serialize = "Mac15,5", to_string = "iMac (24-inch, 2023, Four ports)")]
    Mac15_5,
    #[strum(serialize = "Mac15,4", to_string = "iMac (24-inch, 2023, Two ports)")]
    Mac15_4,
    #[strum(serialize = "iMac21,1", to_string = "iMac (24-inch, M1, 2021)")]
    IMac21_1,
    #[strum(serialize = "iMac21,2", to_string = "iMac (24-inch, M1, 2021)")]
    IMac21_2,
    #[strum(serialize = "iMac20,1", to_string = "iMac (Retina 5K, 27-inch, 2020)")]
    IMac20_1,
    #[strum(serialize = "iMac20,2", to_string = "iMac (Retina 5K, 27-inch, 2020)")]
    IMac20_2,
    #[strum(serialize = "iMac19,1", to_string = "iMac (Retina 5K, 27-inch, 2019)")]
    IMac19_1,
    #[strum(
        serialize = "iMac19,2",
        to_string = "iMac (Retina 4K, 21.5-inch, 2019)"
    )]
    IMac19_2,
    #[strum(serialize = "iMac18,3", to_string = "iMac (Retina 5K, 27-inch, 2017)")]
    IMac18_3,
    #[strum(
        serialize = "iMac18,2",
        to_string = "iMac (Retina 4K, 21.5-inch, 2017)"
    )]
    IMac18_2,
    #[strum(serialize = "iMac18,1", to_string = "iMac (21.5-inch, 2017)")]
    IMac18_1,
    #[strum(
        serialize = "iMac17,1",
        to_string = "iMac (Retina 5K, 27-inch, Late 2015)"
    )]
    IMac17_1,
    #[strum(
        serialize = "iMac16,2",
        to_string = "iMac (Retina 4K, 21.5-inch, Late 2015)"
    )]
    IMac16_2,
    #[strum(serialize = "iMac16,1", to_string = "iMac (21.5-inch, Late 2015)")]
    IMac16_1,
    #[strum(
        serialize = "iMac15,1",
        to_string = "iMac (Retina 5K, 27-inch, Mid 2015)"
    )]
    IMac15_1,
    #[strum(serialize = "iMac14,4", to_string = "iMac (21.5-inch, Mid 2014)")]
    IMac14_4,
    #[strum(serialize = "iMac14,2", to_string = "iMac (27-inch, Late 2013)")]
    IMac14_2,
    #[strum(serialize = "iMac14,1", to_string = "iMac (21.5-inch, Late 2013)")]
    IMac14_1,
    #[strum(serialize = "iMac13,2", to_string = "iMac (27-inch, Late 2012)")]
    IMac13_2,
    #[strum(serialize = "iMac13,1", to_string = "iMac (21.5-inch, Late 2012)")]
    IMac13_1,
    #[strum(serialize = "iMac12,2", to_string = "iMac (27-inch, Mid 2011)")]
    IMac12_2,
    #[strum(serialize = "iMac12,1", to_string = "iMac (21.5-inch, Mid 2011)")]
    IMac12_1,
    #[strum(serialize = "iMac11,3", to_string = "iMac (27-inch, Mid 2010)")]
    IMac11_3,
    #[strum(serialize = "iMac11,2", to_string = "iMac (21.5-inch, Mid 2010)")]
    IMac11_2,
    #[strum(serialize = "iMac10,1", to_string = "iMac (27-inch, Late 2009)")]
    IMac10_1,
    #[strum(serialize = "iMac9,1", to_string = "iMac (24-inch, Early 2009)")]
    IMac9_1,

    // iMac Pro
    #[strum(serialize = "iMacPro1,1", to_string = "iMac Pro (2017)")]
    IMacPro1_1,

    // Mac Pro
    #[strum(serialize = "Mac14,8", to_string = "Mac Pro (2023)")]
    Mac14_8,
    #[strum(serialize = "MacPro7,1", to_string = "Mac Pro (2019)")]
    MacPro7_1,
    #[strum(serialize = "MacPro6,1", to_string = "Mac Pro (Late 2013)")]
    MacPro6_1,
}

impl ModelIdentifier {
    pub fn has_foldable_display(&self) -> bool {
        matches!(
            self.family(),
            Family::MacBookPro | Family::MacBookAir | Family::MacBook
        )
    }

    pub fn has_builtin_mic(&self) -> bool {
        matches!(
            self.family(),
            Family::MacBookPro
                | Family::MacBookAir
                | Family::MacBook
                | Family::IMac
                | Family::IMacPro
        )
    }

    pub fn family(&self) -> Family {
        match self {
            Self::Mac16_11
            | Self::Mac16_10
            | Self::Mac14_3
            | Self::Mac14_12
            | Self::Macmini9_1
            | Self::Macmini8_1
            | Self::Macmini7_1
            | Self::Macmini6_1
            | Self::Macmini6_2
            | Self::Macmini5_1
            | Self::Macmini5_2
            | Self::Macmini4_1
            | Self::Macmini3_1 => Family::MacMini,

            Self::Mac16_9
            | Self::Mac15_14
            | Self::Mac14_13
            | Self::Mac14_14
            | Self::Mac13_1
            | Self::Mac13_2 => Family::MacStudio,

            Self::Mac17_2
            | Self::Mac16_1
            | Self::Mac16_6
            | Self::Mac16_8
            | Self::Mac16_7
            | Self::Mac16_5
            | Self::Mac15_3
            | Self::Mac15_6
            | Self::Mac15_8
            | Self::Mac15_10
            | Self::Mac15_7
            | Self::Mac15_9
            | Self::Mac15_11
            | Self::Mac14_5
            | Self::Mac14_9
            | Self::Mac14_6
            | Self::Mac14_10
            | Self::Mac14_7
            | Self::MacBookPro18_3
            | Self::MacBookPro18_4
            | Self::MacBookPro18_1
            | Self::MacBookPro18_2
            | Self::MacBookPro17_1
            | Self::MacBookPro16_3
            | Self::MacBookPro16_2
            | Self::MacBookPro16_1
            | Self::MacBookPro16_4
            | Self::MacBookPro15_4
            | Self::MacBookPro15_1
            | Self::MacBookPro15_3
            | Self::MacBookPro15_2
            | Self::MacBookPro14_3
            | Self::MacBookPro14_2
            | Self::MacBookPro14_1
            | Self::MacBookPro13_3
            | Self::MacBookPro13_2
            | Self::MacBookPro13_1
            | Self::MacBookPro11_4
            | Self::MacBookPro11_5
            | Self::MacBookPro12_1
            | Self::MacBookPro11_2
            | Self::MacBookPro11_3
            | Self::MacBookPro11_1
            | Self::MacBookPro10_1
            | Self::MacBookPro10_2
            | Self::MacBookPro9_1
            | Self::MacBookPro9_2
            | Self::MacBookPro8_3
            | Self::MacBookPro8_2
            | Self::MacBookPro8_1
            | Self::MacBookPro6_1
            | Self::MacBookPro6_2
            | Self::MacBookPro7_1
            | Self::MacBookPro5_2
            | Self::MacBookPro5_3
            | Self::MacBookPro5_5
            | Self::MacBookPro5_1
            | Self::MacBookPro4_1 => Family::MacBookPro,

            Self::MacBook10_1
            | Self::MacBook9_1
            | Self::MacBook8_1
            | Self::MacBook7_1
            | Self::MacBook6_1
            | Self::MacBook5_2 => Family::MacBook,

            Self::Mac16_13
            | Self::Mac16_12
            | Self::Mac15_13
            | Self::Mac15_12
            | Self::Mac14_15
            | Self::Mac14_2
            | Self::MacBookAir10_1
            | Self::MacBookAir9_1
            | Self::MacBookAir8_2
            | Self::MacBookAir8_1
            | Self::MacBookAir7_2
            | Self::MacBookAir7_1
            | Self::MacBookAir6_2
            | Self::MacBookAir6_1
            | Self::MacBookAir5_2
            | Self::MacBookAir5_1
            | Self::MacBookAir4_2
            | Self::MacBookAir4_1
            | Self::MacBookAir3_2
            | Self::MacBookAir3_1
            | Self::MacBookAir2_1 => Family::MacBookAir,

            Self::Mac16_3
            | Self::Mac16_2
            | Self::Mac15_5
            | Self::Mac15_4
            | Self::IMac21_1
            | Self::IMac21_2
            | Self::IMac20_1
            | Self::IMac20_2
            | Self::IMac19_1
            | Self::IMac19_2
            | Self::IMac18_3
            | Self::IMac18_2
            | Self::IMac18_1
            | Self::IMac17_1
            | Self::IMac16_2
            | Self::IMac16_1
            | Self::IMac15_1
            | Self::IMac14_4
            | Self::IMac14_2
            | Self::IMac14_1
            | Self::IMac13_2
            | Self::IMac13_1
            | Self::IMac12_2
            | Self::IMac12_1
            | Self::IMac11_3
            | Self::IMac11_2
            | Self::IMac10_1
            | Self::IMac9_1 => Family::IMac,

            Self::IMacPro1_1 => Family::IMacPro,

            Self::Mac14_8 | Self::MacPro7_1 | Self::MacPro6_1 => Family::MacPro,
        }
    }

    pub fn current() -> std::io::Result<Option<Self>> {
        let model = hw_model()?;
        Ok(model.parse().ok())
    }
}

#[cfg(test)]
mod tests {}
