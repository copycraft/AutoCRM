//! Reading a VIN (ISO 3779) without asking anyone: whether it is well formed, who made the
//! vehicle (the first three characters, the WMI), and the model year (the tenth). Model
//! names are not in the VIN in a standard way; the optional online lookup adds them.

use serde::Serialize;

/// Letters a VIN never uses: I, O and Q look like 1 and 0.
fn allowed(c: char) -> bool {
    c.is_ascii_digit() || (c.is_ascii_uppercase() && !matches!(c, 'I' | 'O' | 'Q'))
}

/// Upper case, spaces and dashes dropped. None unless 17 valid characters remain.
pub fn normalize(raw: &str) -> Option<String> {
    let vin: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (vin.chars().count() == 17 && vin.chars().all(allowed)).then_some(vin)
}

fn transliterate(c: char) -> u32 {
    match c {
        '0'..='9' => c as u32 - '0' as u32,
        'A' | 'J' => 1,
        'B' | 'K' | 'S' => 2,
        'C' | 'L' | 'T' => 3,
        'D' | 'M' | 'U' => 4,
        'E' | 'N' | 'V' => 5,
        'F' | 'W' => 6,
        'G' | 'P' | 'X' => 7,
        'H' | 'Y' => 8,
        'R' | 'Z' => 9,
        _ => 0,
    }
}

/// The North American check digit (position 9). European makers need not use it, so a
/// mismatch is information, not an error.
pub fn check_digit_ok(vin: &str) -> bool {
    const WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum: u32 = vin
        .chars()
        .zip(WEIGHTS)
        .map(|(c, w)| transliterate(c) * w)
        .sum();
    let expected = match sum % 11 {
        10 => 'X',
        n => char::from_digit(n, 10).unwrap_or('?'),
    };
    vin.chars().nth(8) == Some(expected)
}

/// The model year from position 10. The code repeats every 30 years; the later year not
/// after `max_year` (next year, as a new model year starts early) wins.
pub fn model_year(vin: &str, max_year: i32) -> Option<i32> {
    const CODES: &str = "ABCDEFGHJKLMNPRSTVWXY123456789";
    let c = vin.chars().nth(9)?;
    let index = CODES.find(c)? as i32;
    let mut year = 1980 + index;
    while year + 30 <= max_year {
        year += 30;
    }
    Some(year)
}

/// Manufacturers by WMI, as they appear on vans, trucks and cars in a Hungarian workshop.
/// Three-letter codes first, then two-letter prefixes.
const MAKERS: &[(&str, &str)] = &[
    ("W1V", "Mercedes-Benz (kishaszonjármű)"),
    ("W1T", "Mercedes-Benz (tehergépjármű)"),
    ("W1K", "Mercedes-Benz"),
    ("W1N", "Mercedes-Benz"),
    ("WDB", "Mercedes-Benz"),
    ("WDD", "Mercedes-Benz"),
    ("WDF", "Mercedes-Benz (kishaszonjármű)"),
    ("WD3", "Mercedes-Benz (Sprinter)"),
    ("WV1", "Volkswagen haszonjármű"),
    ("WV2", "Volkswagen haszonjármű"),
    ("WV3", "Volkswagen haszonjármű"),
    ("WVW", "Volkswagen"),
    ("WVG", "Volkswagen"),
    ("WF0", "Ford (Németország)"),
    ("NM0", "Ford (Ford Otosan, Törökország)"),
    ("WMA", "MAN"),
    ("WMM", "MAN"),
    ("WAU", "Audi"),
    ("WBA", "BMW"),
    ("W0L", "Opel"),
    ("W0V", "Opel"),
    ("VF1", "Renault"),
    ("VF6", "Renault Trucks"),
    ("VXE", "Opel / Vauxhall (Stellantis)"),
    ("VXK", "Opel / Vauxhall (Stellantis)"),
    ("VR1", "DS Automobiles"),
    ("YAR", "Toyota (Stellantis-gyártás)"),
    ("VF3", "Peugeot"),
    ("VR3", "Peugeot"),
    ("VF7", "Citroën"),
    ("VR7", "Citroën"),
    ("VSS", "SEAT"),
    ("VNK", "Toyota (Franciaország)"),
    ("VSK", "Nissan (Spanyolország)"),
    ("ZFA", "Fiat"),
    ("ZCF", "Iveco"),
    ("WJM", "Iveco"),
    ("YS2", "Scania"),
    ("YV2", "Volvo Trucks"),
    ("YV1", "Volvo"),
    ("XLR", "DAF"),
    ("TMB", "Škoda"),
    ("SB1", "Toyota (Egyesült Királyság)"),
    ("SJN", "Nissan (Egyesült Királyság)"),
    ("KMH", "Hyundai"),
    ("KNA", "Kia"),
    ("U5Y", "Kia (Szlovákia)"),
    ("JAL", "Isuzu"),
    ("JAA", "Isuzu"),
    ("JMB", "Mitsubishi"),
    ("JT", "Toyota"),
    ("JN", "Nissan"),
    ("LS", "SAIC / Maxus"),
    ("TR", "Magyarország (pl. Suzuki)"),
];

pub fn manufacturer(vin: &str) -> Option<&'static str> {
    let wmi = vin.get(..3)?;
    MAKERS
        .iter()
        .find(|(code, _)| code.len() == 3 && *code == wmi)
        .or_else(|| {
            MAKERS
                .iter()
                .find(|(code, _)| code.len() == 2 && wmi.starts_with(code))
        })
        .map(|(_, name)| *name)
}

/// The region the first character assigns.
pub fn region(vin: &str) -> Option<&'static str> {
    Some(match vin.chars().next()? {
        'A'..='H' => "Afrika",
        'J'..='R' => "Ázsia",
        'S'..='Z' => "Európa",
        '1'..='5' => "Észak-Amerika",
        '6' | '7' => "Óceánia",
        '8' | '9' => "Dél-Amerika",
        _ => return None,
    })
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct VinInfo {
    pub vin: String,
    /// The North American check digit matches. European makers often do not use it.
    pub check_digit_ok: bool,
    pub manufacturer: Option<String>,
    pub region: Option<String>,
    pub model_year: Option<i32>,
    /// Filled by the online lookup only.
    pub make: Option<String>,
    /// The model family: read offline for the common European vans (Sprinter, Crafter,
    /// Ducato, Vivaro…), or from the online lookup.
    pub model: Option<String>,
    /// "offline", or "nhtsa" when the online lookup added to it.
    pub source: String,
}

/// The van and truck families this workshop sees most, read from where each maker puts
/// the model in the VIN. A hint, not a type approval: the exact variant and engine are only
/// in the maker's own records. (prefix to match from the start, family)
const FAMILIES: &[(&str, &str)] = &[
    // Mercedes-Benz: the model series sits in positions 4–6.
    ("WDB906", "Sprinter (NCV3, 2006–2018)"),
    ("WDA906", "Sprinter (NCV3, 2006–2018)"),
    ("WDB907", "Sprinter (VS30, 2018–)"),
    ("W1V907", "Sprinter (VS30, 2018–)"),
    ("W1V910", "Sprinter (VS30, fronthajtású, 2018–)"),
    ("WDB910", "Sprinter (VS30, fronthajtású, 2018–)"),
    ("WDF639", "Vito / Viano (W639)"),
    ("WDF447", "Vito (W447)"),
    ("W1V447", "Vito (W447)"),
    ("WDF415", "Citan (W415)"),
    // Volkswagen: positions 7–8 after the ZZZ filler.
    ("WV1ZZZ2E", "Crafter (2006–2016)"),
    ("WV2ZZZ2E", "Crafter (2006–2016)"),
    ("WV1ZZZSY", "Crafter (2017–)"),
    ("WV1ZZZSZ", "Crafter (2017–)"),
    ("WV2ZZZSY", "Crafter (2017–)"),
    ("WV1ZZZ7H", "Transporter T5"),
    ("WV2ZZZ7H", "Transporter T5"),
    ("WV1ZZZ7J", "Transporter T6"),
    ("WV2ZZZ7J", "Transporter T6"),
    ("WV1ZZZST", "Transporter T6.1"),
    ("WV1ZZZ2K", "Caddy (2K)"),
    ("WV1ZZZSK", "Caddy (2020–)"),
    // Fiat / Iveco.
    ("ZFA250", "Ducato (2006–)"),
    ("ZFA244", "Ducato (2002–2006)"),
    ("ZFA263", "Doblò (2010–)"),
    ("ZCFC", "Daily"),
    // Stellantis vans: the K0 mid-size van (Expert / Jumpy / Vivaro-C / ProAce) carries a V
    // in position 4, the large van (Boxer / Jumper) a Y.
    ("VF3V", "Expert (K0, 2016–)"),
    ("VF7V", "Jumpy (K0, 2016–)"),
    ("VXEV", "Vivaro (Vivaro-C, K0 – mint Expert / Jumpy / ProAce, 2019–)"),
    ("YARV", "ProAce (K0 – mint Expert / Jumpy / Vivaro)"),
    ("VF3Y", "Boxer"),
    ("VF7Y", "Jumper"),
    ("VXEY", "Movano (Movano-C, mint Boxer / Jumper / Ducato, 2021–)"),
    ("VXEE", "Combo (Combo-E, K9 – mint Partner / Berlingo)"),
    ("VF37", "Partner / Rifter (K9)"),
    ("VF7E", "Berlingo (K9)"),
    // Renault.
    ("VF1MA", "Master"),
    ("VF1FL", "Trafic"),
    ("VF1JL", "Trafic"),
    // Ford Europe: the model in positions 7–8 after the XXX filler.
    ("WF0XXXTTG", "Transit (2014–)"),
    ("WF0XXXTTF", "Transit (2006–2014)"),
    ("WF0XXXTTR", "Transit Custom"),
    ("NM0XXXTTG", "Transit Custom (Ford Otosan)"),
];

pub fn family(vin: &str) -> Option<&'static str> {
    FAMILIES
        .iter()
        .filter(|(prefix, _)| vin.starts_with(prefix))
        // The longest prefix is the most specific.
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, name)| *name)
}

pub fn decode(vin: &str, max_year: i32) -> Option<VinInfo> {
    let vin = normalize(vin)?;
    Some(VinInfo {
        check_digit_ok: check_digit_ok(&vin),
        manufacturer: manufacturer(&vin).map(str::to_string),
        region: region(&vin).map(str::to_string),
        model_year: model_year(&vin, max_year),
        make: None,
        // The family the VIN shows, when it is one we know; the online lookup can refine it.
        model: family(&vin).map(str::to_string),
        source: "offline".into(),
        vin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_rejects() {
        assert_eq!(
            normalize(" wdb-906 6351s123456 ").as_deref(),
            Some("WDB9066351S123456")
        );
        assert!(normalize("WDB9066351S12345").is_none(), "16 characters");
        assert!(normalize("WDB9066351O123456").is_none(), "O is not used");
    }

    #[test]
    fn the_check_digit_of_a_known_vin() {
        // The textbook example (Wikipedia, "Vehicle identification number").
        assert!(check_digit_ok("1M8GDM9AXKP042788"));
        assert!(!check_digit_ok("1M8GDM9A1KP042788"));
    }

    #[test]
    fn model_years_pick_the_latest_cycle() {
        // 'K' is 1989 or 2019; 'A' is 1980, 2010 or 2040.
        assert_eq!(model_year("WDB906635K1234567", 2027), Some(2019));
        assert_eq!(model_year("WDB906635A1234567", 2027), Some(2010));
        assert_eq!(model_year("WDB90663591234567", 2027), Some(2009));
        assert_eq!(model_year("WDB906635T1234567", 2027), Some(2026));
    }

    #[test]
    fn european_van_families_read_offline() {
        // A real Vivaro-C: Stellantis' Opel code, the K0 van's V, model year L = 2020.
        let info = decode("VXEVBYHRKL7007599", 2027).unwrap();
        assert_eq!(info.manufacturer.as_deref(), Some("Opel / Vauxhall (Stellantis)"));
        assert!(info.model.as_deref().unwrap().starts_with("Vivaro"));
        assert_eq!(info.model_year, Some(2020));
        assert_eq!(family("WDB9066351S123456"), Some("Sprinter (NCV3, 2006–2018)"));
        assert_eq!(family("WV1ZZZSYZJ9012345"), Some("Crafter (2017–)"));
        assert_eq!(family("ABC12345678901234"), None);
    }

    #[test]
    fn makers_by_three_then_two_characters() {
        let info = decode("WDB9066351S123456", 2027).unwrap();
        assert_eq!(info.manufacturer.as_deref(), Some("Mercedes-Benz"));
        assert_eq!(info.region.as_deref(), Some("Európa"));
        assert_eq!(manufacturer("JTDKB20U403123456"), Some("Toyota"));
        assert_eq!(manufacturer("ABC12345678901234"), None);
    }
}
