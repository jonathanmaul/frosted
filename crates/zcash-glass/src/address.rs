//! Payout Unified Address validation. Receivers remain opaque protocol bytes.
use thiserror::Error;
use zcash_address::unified::{Address, Container, Encoding, Receiver};
use zcash_protocol::consensus::NetworkType;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayoutAddressError {
    #[error("PAYOUT_ADDRESS_NO_ORCHARD")]
    NoOrchard,
    #[error("PAYOUT_ADDRESS_WRONG_NETWORK")]
    WrongNetwork,
}

pub fn parse_payout_ua(
    ua: &str,
    network: &str,
) -> Result<crate::OrchardReceiver, PayoutAddressError> {
    let expected = match network {
        "mainnet" => NetworkType::Main,
        "testnet" => NetworkType::Test,
        "regtest" => NetworkType::Regtest,
        _ => return Err(PayoutAddressError::WrongNetwork),
    };
    let (actual, address) = Address::decode(ua).map_err(|_| PayoutAddressError::NoOrchard)?;
    if actual != expected {
        return Err(PayoutAddressError::WrongNetwork);
    }
    address
        .items()
        .into_iter()
        .find_map(|receiver| match receiver {
            Receiver::Orchard(raw) => Some(raw),
            _ => None,
        })
        .ok_or(PayoutAddressError::NoOrchard)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_network_and_receiver() {
        let ua = Address::try_from_items(vec![Receiver::Orchard([7; 43])]).unwrap();
        for (network, name) in [
            (NetworkType::Main, "mainnet"),
            (NetworkType::Test, "testnet"),
            (NetworkType::Regtest, "regtest"),
        ] {
            assert_eq!(parse_payout_ua(&ua.encode(&network), name), Ok([7; 43]));
        }
        assert_eq!(
            parse_payout_ua(&ua.encode(&NetworkType::Main), "testnet"),
            Err(PayoutAddressError::WrongNetwork)
        );
        let sapling = Address::try_from_items(vec![Receiver::Sapling([7; 43])]).unwrap();
        assert_eq!(
            parse_payout_ua(&sapling.encode(&NetworkType::Test), "testnet"),
            Err(PayoutAddressError::NoOrchard)
        );
        assert_eq!(
            parse_payout_ua("invalid", "testnet"),
            Err(PayoutAddressError::NoOrchard)
        );
    }
}
