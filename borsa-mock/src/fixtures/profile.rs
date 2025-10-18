use borsa_core::{CompanyProfile, Profile};

pub fn by_symbol(s: &str) -> Option<Profile> {
    let (name, sector, industry) = if s == "META" {
        (
            "Meta Platforms, Inc.",
            "Communication Services",
            "Interactive Media",
        )
    } else {
        ("Generic Corp", "Technology", "Software")
    };
    Some(Profile::Company(CompanyProfile {
        name: name.to_string(),
        website: None,
        summary: None,
        address: None,
        sector: Some(sector.to_string()),
        industry: Some(industry.to_string()),
        isin: None,
    }))
}
