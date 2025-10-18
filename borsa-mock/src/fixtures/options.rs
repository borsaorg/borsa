use borsa_core::OptionChain;

pub fn expirations_by_symbol(_s: &str) -> Vec<i64> {
    vec![1_700_000_000]
}

pub const fn chain_by_symbol_and_date(_s: &str, _d: Option<i64>) -> Option<OptionChain> {
    Some(OptionChain {
        calls: vec![],
        puts: vec![],
    })
}
