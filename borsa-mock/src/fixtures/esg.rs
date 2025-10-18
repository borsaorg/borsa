use borsa_core::EsgScores;

pub const fn by_symbol(_s: &str) -> Option<EsgScores> {
    Some(EsgScores {
        environmental: None,
        social: None,
        governance: None,
    })
}
