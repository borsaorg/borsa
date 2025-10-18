use borsa_core::EsgScores;

pub const fn by_symbol(_s: &str) -> EsgScores {
    EsgScores {
        environmental: None,
        social: None,
        governance: None,
    }
}
