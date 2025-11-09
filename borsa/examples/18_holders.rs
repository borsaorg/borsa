mod common;
use borsa::Borsa;
use borsa_core::{AssetKind, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connector = common::get_connector();
    let borsa = Borsa::builder().with_connector(connector).build()?;

    let inst = Instrument::from_symbol("AAPL", AssetKind::Equity)?;
    let sym_str = match inst.id() {
        borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str(),
        borsa_core::IdentifierScheme::Prediction(_) => "<non-security>",
    };

    println!("Fetching holders and insider activity for {sym_str}...");

    let major = borsa.major_holders(&inst).await?;
    println!("major holders rows: {}", major.len());

    let inst_holders = borsa.institutional_holders(&inst).await?;
    println!("institutional holders rows: {}", inst_holders.len());

    let fund_holders = borsa.mutual_fund_holders(&inst).await?;
    println!("mutual fund holders rows: {}", fund_holders.len());

    let insider_tx = borsa.insider_transactions(&inst).await?;
    println!("insider transactions: {}", insider_tx.len());

    let roster = borsa.insider_roster_holders(&inst).await?;
    println!("insider roster entries: {}", roster.len());

    let net = borsa.net_share_purchase_activity(&inst).await?;
    println!(
        "net share purchase activity present: {}",
        net.as_ref().is_some_and(|_| true)
    );

    Ok(())
}
