//! Channel layouts: ffmpeg layout names -> ordered channel ids, labels, roles.

/// A resolved channel layout for a decoded stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub name: String,
    pub ids: Vec<String>,
    /// True when the source did not state a layout and we guessed from the channel count.
    pub assumed: bool,
}

fn named(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "mono" => &["FC"],
        "stereo" => &["FL", "FR"],
        "2.1" => &["FL", "FR", "LFE"],
        "3.0" => &["FL", "FR", "FC"],
        "3.0(back)" => &["FL", "FR", "BC"],
        "3.1" => &["FL", "FR", "FC", "LFE"],
        "4.0" => &["FL", "FR", "FC", "BC"],
        "quad" => &["FL", "FR", "BL", "BR"],
        "quad(side)" => &["FL", "FR", "SL", "SR"],
        "4.1" => &["FL", "FR", "FC", "LFE", "BC"],
        "5.0" => &["FL", "FR", "FC", "BL", "BR"],
        "5.0(side)" => &["FL", "FR", "FC", "SL", "SR"],
        "5.1" => &["FL", "FR", "FC", "LFE", "BL", "BR"],
        "5.1(side)" => &["FL", "FR", "FC", "LFE", "SL", "SR"],
        "6.0" => &["FL", "FR", "FC", "BC", "SL", "SR"],
        "6.1" => &["FL", "FR", "FC", "LFE", "BC", "SL", "SR"],
        "6.1(back)" => &["FL", "FR", "FC", "LFE", "BL", "BR", "BC"],
        "7.0" => &["FL", "FR", "FC", "BL", "BR", "SL", "SR"],
        "7.1" => &["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR"],
        "7.1(wide)" => &["FL", "FR", "FC", "LFE", "BL", "BR", "FLC", "FRC"],
        "7.1(wide-side)" => &["FL", "FR", "FC", "LFE", "FLC", "FRC", "SL", "SR"],
        _ => return None,
    })
}

/// The layout ffmpeg assumes for a bare channel count.
pub fn default_for(channels: u32) -> Option<&'static str> {
    Some(match channels {
        1 => "mono",
        2 => "stereo",
        3 => "2.1",
        4 => "4.0",
        5 => "5.0",
        6 => "5.1",
        7 => "6.1",
        8 => "7.1",
        _ => return None,
    })
}

/// Parses "FL+FR+FC" or "6 channels (FL+FR+FC+LFE+SL+SR)".
fn parse_custom(s: &str, channels: u32) -> Option<Vec<String>> {
    let inner = match (s.find('('), s.rfind(')')) {
        (Some(a), Some(b)) if b > a => &s[a + 1..b],
        _ => s,
    };
    if !inner.contains('+') {
        return None;
    }
    let ids: Vec<String> = inner.split('+').map(|t| t.trim().to_string()).collect();
    (ids.len() == channels as usize && ids.iter().all(|t| !t.is_empty())).then_some(ids)
}

pub fn resolve(layout: Option<&str>, channels: u32) -> Layout {
    let raw = layout.map(str::trim).unwrap_or_default();
    if let Some(ids) = named(raw) {
        if ids.len() == channels as usize {
            return Layout { name: raw.into(), ids: ids.iter().map(|s| s.to_string()).collect(), assumed: false };
        }
    }
    if let Some(ids) = parse_custom(raw, channels) {
        return Layout { name: raw.into(), ids, assumed: false };
    }
    if let Some(name) = default_for(channels) {
        let ids = named(name).unwrap();
        return Layout { name: name.into(), ids: ids.iter().map(|s| s.to_string()).collect(), assumed: true };
    }
    Layout {
        name: format!("{channels} channels"),
        ids: (1..=channels).map(|i| format!("CH{i}")).collect(),
        assumed: true,
    }
}

fn has(all: &[String], id: &str) -> bool {
    all.iter().any(|x| x == id)
}

/// Layout-independent role used by presets. In back-surround 5.1 ("5.1": BL/BR,
/// no SL/SR) the back pair *is* the surround pair, so it maps to SL/SR.
pub fn role(id: &str, all: &[String]) -> String {
    match id {
        "BL" if !has(all, "SL") => "SL".into(),
        "BR" if !has(all, "SR") => "SR".into(),
        _ => id.into(),
    }
}

pub fn label(id: &str, all: &[String]) -> String {
    let l = match id {
        "FL" => "Front Left",
        "FR" => "Front Right",
        "FC" => "Center",
        "LFE" => "LFE / Subwoofer",
        "SL" => "Surround Left",
        "SR" => "Surround Right",
        "BL" if !has(all, "SL") => "Surround Left",
        "BR" if !has(all, "SR") => "Surround Right",
        "BL" => "Back Left",
        "BR" => "Back Right",
        "BC" => "Back Center",
        "FLC" => "Front Left of Center",
        "FRC" => "Front Right of Center",
        "TC" => "Top Center",
        "TFL" => "Top Front Left",
        "TFR" => "Top Front Right",
        _ => return id.to_string(),
    };
    l.to_string()
}

/// Channel id that is on the left/right/centre side, used for default routing.
pub enum Side {
    Left,
    Right,
    Center,
}

pub fn side(id: &str) -> Side {
    match id {
        "FL" | "SL" | "BL" | "FLC" | "TFL" | "TBL" | "WL" => Side::Left,
        "FR" | "SR" | "BR" | "FRC" | "TFR" | "TBR" | "WR" => Side::Right,
        _ => Side::Center,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_layouts() {
        let l = resolve(Some("5.1(side)"), 6);
        assert_eq!(l.ids, ["FL", "FR", "FC", "LFE", "SL", "SR"]);
        assert!(!l.assumed);
    }

    #[test]
    fn back_surround_maps_to_surround_role() {
        let l = resolve(Some("5.1"), 6);
        assert_eq!(role("BL", &l.ids), "SL");
        assert_eq!(role("BR", &l.ids), "SR");
        assert_eq!(label("BL", &l.ids), "Surround Left");
        let l71 = resolve(Some("7.1"), 8);
        assert_eq!(role("BL", &l71.ids), "BL");
        assert_eq!(role("SL", &l71.ids), "SL");
    }

    #[test]
    fn custom_and_unknown() {
        let l = resolve(Some("6 channels (FL+FR+FC+LFE+SL+SR)"), 6);
        assert_eq!(l.ids[4], "SL");
        assert!(!l.assumed);
        let u = resolve(Some(""), 6);
        assert!(u.assumed);
        assert_eq!(u.name, "5.1");
        let odd = resolve(None, 12);
        assert_eq!(odd.ids.len(), 12);
    }
}
