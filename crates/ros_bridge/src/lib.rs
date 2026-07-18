//! Versioned contracts for boundaries between Flutter, Rust, and cloud services.

#![forbid(unsafe_code)]

pub const CLIENT_CORE_CONTRACT_VERSION: u16 = 1;
pub const MINIMUM_SUPPORTED_CLIENT_CORE_CONTRACT_VERSION: u16 = 1;

pub const fn is_supported_client_core_contract(version: u16) -> bool {
    version >= MINIMUM_SUPPORTED_CLIENT_CORE_CONTRACT_VERSION
        && version <= CLIENT_CORE_CONTRACT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_contract_is_supported() {
        assert!(is_supported_client_core_contract(
            CLIENT_CORE_CONTRACT_VERSION
        ));
        assert!(!is_supported_client_core_contract(0));
        assert!(!is_supported_client_core_contract(
            CLIENT_CORE_CONTRACT_VERSION + 1
        ));
    }
}
