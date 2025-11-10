use crate::helpers::{MockConnector, X, dt, ts, usd};
use borsa::Borsa;
use borsa_core::{AssetKind, OptionChain, OptionContract};

#[tokio::test]
async fn option_chain_falls_back_and_succeeds() {
    let err = MockConnector::builder()
        .name("err_chain")
        .with_option_chain_fn(|_i, _d| Err(borsa_core::BorsaError::unsupported("options/chain")))
        .build();
    let ok = MockConnector::builder()
        .name("ok_chain")
        .with_option_chain_fn(|_i, date| {
            assert_eq!(date, Some(ts(2024, 6, 25, 0, 0, 0)));
            Ok(OptionChain {
                calls: vec![OptionContract {
                    instrument: borsa_core::Instrument::from_symbol(
                        "X250620C00050000",
                        AssetKind::Equity,
                    )
                    .unwrap(),
                    strike: usd("50.0"),
                    price: Some(usd("2.5")),
                    bid: Some(usd("2.4")),
                    ask: Some(usd("2.6")),
                    volume: Some(10),
                    open_interest: Some(100),
                    implied_volatility: Some(0.4),
                    in_the_money: false,
                    expiration_at: Some(dt(2024, 6, 25, 0, 0, 0)),
                    expiration_date: chrono::NaiveDate::from_ymd_opt(2024, 6, 25).unwrap(),
                    greeks: None,
                    last_trade_at: None,
                }],
                puts: vec![],
            })
        })
        .build();

    let borsa = Borsa::builder()
        .with_connector(err)
        .with_connector(ok)
        .build()
        .unwrap();

    let inst = crate::helpers::instrument(&X, AssetKind::Equity);
    let ch = borsa
        .option_chain(&inst, Some(ts(2024, 6, 25, 0, 0, 0)))
        .await
        .unwrap();

    assert_eq!(ch.calls.len(), 1);
    assert_eq!(
        ch.calls[0].strike.amount(),
        rust_decimal::Decimal::from(50u8)
    );
}
