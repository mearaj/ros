//! Provider-neutral verification for short-lived Professional device tokens.
//!
//! Identity issuance may later be backed by a managed provider, but the API
//! accepts only locally verified Ed25519 JWTs with a narrow, versioned claim
//! contract. Raw bearer values and key material are never included in errors.

use std::{fmt, sync::Arc};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use ros_core::EntityId;
use serde::Deserialize;
use time::OffsetDateTime;

const DEVICE_TOKEN_VERSION: u8 = 1;
const MAX_BEARER_TOKEN_BYTES: usize = 8_192;
const MAX_CONFIGURED_ISSUER_BYTES: usize = 2_048;
const MAX_CONFIGURED_AUDIENCE_BYTES: usize = 256;
const MAX_DEVICE_TOKEN_LIFETIME_SECONDS: i64 = 15 * 60;
const CLOCK_SKEW_SECONDS: i64 = 30;
pub const SYNC_WRITE_PERMISSION: &str = "sync:write";

#[derive(Clone)]
pub struct DeviceTokenVerifier {
    decoding_key: Arc<DecodingKey>,
    validation: Validation,
}

impl DeviceTokenVerifier {
    pub fn from_ed25519_pem(
        public_key_pem: &[u8],
        issuer: &str,
        audience: &str,
    ) -> Result<Self, DeviceAuthError> {
        if !valid_configured_claim(issuer, MAX_CONFIGURED_ISSUER_BYTES)
            || !valid_configured_claim(audience, MAX_CONFIGURED_AUDIENCE_BYTES)
        {
            return Err(DeviceAuthError::ConfigurationInvalid);
        }
        let decoding_key = DecodingKey::from_ed_pem(public_key_pem)
            .map_err(|_| DeviceAuthError::ConfigurationInvalid)?;
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "nbf", "sub"]);
        validation.leeway = CLOCK_SKEW_SECONDS as u64;
        validation.validate_exp = true;
        validation.validate_nbf = true;

        Ok(Self {
            decoding_key: Arc::new(decoding_key),
            validation,
        })
    }

    pub fn verify_bearer(
        &self,
        authorization: Option<&str>,
    ) -> Result<DeviceClaims, DeviceAuthError> {
        let authorization = authorization.ok_or(DeviceAuthError::Missing)?;
        let token = authorization
            .strip_prefix("Bearer ")
            .ok_or(DeviceAuthError::Invalid)?;
        if token.is_empty()
            || token.len() > MAX_BEARER_TOKEN_BYTES
            || token.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(DeviceAuthError::Invalid);
        }

        let header = decode_header(token).map_err(|_| DeviceAuthError::Invalid)?;
        if header.alg != Algorithm::EdDSA
            || header.typ.as_deref() != Some("JWT")
            || header.crit.is_some()
            || header.jku.is_some()
            || header.jwk.is_some()
            || header.x5u.is_some()
            || header.x5c.is_some()
        {
            return Err(DeviceAuthError::Invalid);
        }

        let token = decode::<RawDeviceClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|_| DeviceAuthError::Invalid)?;
        DeviceClaims::try_from(token.claims)
    }
}

fn valid_configured_claim(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceClaims {
    organization_id: EntityId,
    branch_id: EntityId,
    device_id: EntityId,
    token_id: EntityId,
}

impl DeviceClaims {
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }

    pub fn device_id(&self) -> &EntityId {
        &self.device_id
    }

    pub fn token_id(&self) -> &EntityId {
        &self.token_id
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RawDeviceClaims {
    sub: String,
    organization_id: String,
    branch_id: String,
    device_id: String,
    jti: String,
    permissions: Vec<String>,
    token_version: u8,
    iat: i64,
    nbf: i64,
    exp: i64,
}

impl TryFrom<RawDeviceClaims> for DeviceClaims {
    type Error = DeviceAuthError;

    fn try_from(value: RawDeviceClaims) -> Result<Self, Self::Error> {
        if value.token_version != DEVICE_TOKEN_VERSION || value.permissions.len() > 32 {
            return Err(DeviceAuthError::Invalid);
        }
        for (index, permission) in value.permissions.iter().enumerate() {
            if permission.is_empty()
                || permission.len() > 100
                || !permission.bytes().all(|byte| byte.is_ascii_graphic())
                || value.permissions[..index].contains(permission)
            {
                return Err(DeviceAuthError::Invalid);
            }
        }
        if !value
            .permissions
            .iter()
            .any(|permission| permission == SYNC_WRITE_PERMISSION)
        {
            return Err(DeviceAuthError::Forbidden);
        }
        if value.iat > value.nbf
            || value.nbf > value.exp
            || value.exp.saturating_sub(value.iat) > MAX_DEVICE_TOKEN_LIFETIME_SECONDS
        {
            return Err(DeviceAuthError::Invalid);
        }
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if value.iat > now.saturating_add(CLOCK_SKEW_SECONDS)
            || value.exp < now.saturating_sub(CLOCK_SKEW_SECONDS)
        {
            return Err(DeviceAuthError::Invalid);
        }

        let organization_id = parse_canonical_id(&value.organization_id)?;
        let branch_id = parse_canonical_id(&value.branch_id)?;
        let device_id = parse_canonical_id(&value.device_id)?;
        let token_id = parse_canonical_id(&value.jti)?;
        if value.sub != value.device_id {
            return Err(DeviceAuthError::Invalid);
        }

        Ok(Self {
            organization_id,
            branch_id,
            device_id,
            token_id,
        })
    }
}

fn parse_canonical_id(value: &str) -> Result<EntityId, DeviceAuthError> {
    if value.len() != 36 {
        return Err(DeviceAuthError::Invalid);
    }
    EntityId::parse(value).map_err(|_| DeviceAuthError::Invalid)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceAuthError {
    ConfigurationInvalid,
    Missing,
    Invalid,
    Forbidden,
}

impl fmt::Display for DeviceAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ConfigurationInvalid => "device-token verification is not configured",
            Self::Missing => "a device access token is required",
            Self::Invalid => "the device access token is invalid",
            Self::Forbidden => "the device access token lacks sync permission",
        })
    }
}

impl std::error::Error for DeviceAuthError {}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::OnceLock;

    use ed25519_dalek::{
        SigningKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding},
    };
    use jsonwebtoken::EncodingKey;
    use rand_core::OsRng;

    pub(crate) struct TestEd25519Keys {
        encoding_key: EncodingKey,
        public_key_pem: String,
    }

    impl TestEd25519Keys {
        pub(crate) fn encoding_key(&self) -> &EncodingKey {
            &self.encoding_key
        }

        pub(crate) fn public_key_pem(&self) -> &[u8] {
            self.public_key_pem.as_bytes()
        }
    }

    pub(crate) fn test_ed25519_keys() -> &'static TestEd25519Keys {
        static KEYS: OnceLock<TestEd25519Keys> = OnceLock::new();
        KEYS.get_or_init(|| {
            let signing_key = SigningKey::generate(&mut OsRng);
            let private_key_pem = signing_key
                .to_pkcs8_pem(LineEnding::LF)
                .expect("runtime test key encodes as PKCS#8");
            let encoding_key = EncodingKey::from_ed_pem(private_key_pem.as_bytes())
                .expect("runtime test key is accepted by the JWT encoder");
            let public_key_pem = signing_key
                .verifying_key()
                .to_public_key_pem(LineEnding::LF)
                .expect("runtime test public key encodes as SubjectPublicKeyInfo");
            TestEd25519Keys {
                encoding_key,
                public_key_pem,
            }
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde::Serialize;

    use super::test_support::test_ed25519_keys;
    use super::*;

    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        organization_id: String,
        branch_id: String,
        device_id: String,
        jti: String,
        permissions: Vec<String>,
        token_version: u8,
        iss: &'static str,
        aud: &'static str,
        iat: i64,
        nbf: i64,
        exp: i64,
    }

    fn test_claims(
        organization_id: &EntityId,
        branch_id: &EntityId,
        device_id: &EntityId,
        permissions: Vec<String>,
    ) -> TestClaims {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        TestClaims {
            sub: device_id.to_string(),
            organization_id: organization_id.to_string(),
            branch_id: branch_id.to_string(),
            device_id: device_id.to_string(),
            jti: EntityId::new_v7().to_string(),
            permissions,
            token_version: DEVICE_TOKEN_VERSION,
            iss: "https://identity.test.gotigin.invalid",
            aud: "restaurant-os-professional-api",
            iat: now,
            nbf: now,
            exp: now + 300,
        }
    }

    fn encode_ed25519(claims: &TestClaims, header: &Header) -> String {
        encode(header, claims, test_ed25519_keys().encoding_key()).expect("signed test token")
    }

    pub(crate) fn signed_token(
        organization_id: &EntityId,
        branch_id: &EntityId,
        device_id: &EntityId,
        permissions: Vec<String>,
    ) -> String {
        encode_ed25519(
            &test_claims(organization_id, branch_id, device_id, permissions),
            &Header::new(Algorithm::EdDSA),
        )
    }

    fn verifier() -> DeviceTokenVerifier {
        DeviceTokenVerifier::from_ed25519_pem(
            test_ed25519_keys().public_key_pem(),
            "https://identity.test.gotigin.invalid",
            "restaurant-os-professional-api",
        )
        .expect("test verifier")
    }

    #[test]
    fn verifies_a_short_lived_tenant_scoped_device_token() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let token = signed_token(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );

        let claims = verifier()
            .verify_bearer(Some(&format!("Bearer {token}")))
            .expect("valid token");
        assert_eq!(claims.organization_id(), &organization_id);
        assert_eq!(claims.branch_id(), &branch_id);
        assert_eq!(claims.device_id(), &device_id);
    }

    #[test]
    fn rejects_missing_permissions_wrong_schemes_and_noncanonical_identity() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let token = signed_token(&organization_id, &branch_id, &device_id, Vec::new());
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Forbidden)
        );
        assert_eq!(
            verifier().verify_bearer(Some("Basic value")),
            Err(DeviceAuthError::Invalid)
        );
        assert_eq!(
            verifier().verify_bearer(None),
            Err(DeviceAuthError::Missing)
        );

        let mut claims = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        claims.organization_id.push(' ');
        let token = encode_ed25519(&claims, &Header::new(Algorithm::EdDSA));
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );
    }

    #[test]
    fn rejects_algorithm_confusion_and_untrusted_key_discovery_headers() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let claims = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        let hmac_token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"not-an-ed25519-key"),
        )
        .expect("HMAC test token");
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {hmac_token}"))),
            Err(DeviceAuthError::Invalid)
        );

        let mut untrusted_header = Header::new(Algorithm::EdDSA);
        untrusted_header.jku = Some("https://attacker.invalid/keys.json".to_owned());
        let token = encode_ed25519(&claims, &untrusted_header);
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );
    }

    #[test]
    fn rejects_wrong_scope_subject_and_excessive_lifetime() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();
        let mut claims = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        claims.iss = "https://wrong-issuer.test.invalid";
        let token = encode_ed25519(&claims, &Header::new(Algorithm::EdDSA));
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );

        let mut claims = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        claims.sub = EntityId::new_v7().to_string();
        let token = encode_ed25519(&claims, &Header::new(Algorithm::EdDSA));
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );

        let mut claims = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        claims.exp = claims.iat + MAX_DEVICE_TOKEN_LIFETIME_SECONDS + 1;
        let token = encode_ed25519(&claims, &Header::new(Algorithm::EdDSA));
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );
    }

    #[test]
    fn verifier_configuration_rejects_ambiguous_claim_namespaces_and_wrong_keys() {
        assert!(matches!(
            DeviceTokenVerifier::from_ed25519_pem(
                test_ed25519_keys().public_key_pem(),
                " https://identity.test.gotigin.invalid",
                "restaurant-os-professional-api",
            ),
            Err(DeviceAuthError::ConfigurationInvalid)
        ));
        assert!(matches!(
            DeviceTokenVerifier::from_ed25519_pem(
                b"not a PEM key",
                "https://identity.test.gotigin.invalid",
                "restaurant-os-professional-api",
            ),
            Err(DeviceAuthError::ConfigurationInvalid)
        ));
    }

    #[test]
    fn distinguishes_invalid_claim_shape_from_insufficient_permission() {
        let organization_id = EntityId::new_v7();
        let branch_id = EntityId::new_v7();
        let device_id = EntityId::new_v7();

        let mut wrong_version = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![SYNC_WRITE_PERMISSION.to_owned()],
        );
        wrong_version.token_version = DEVICE_TOKEN_VERSION + 1;
        let token = encode_ed25519(&wrong_version, &Header::new(Algorithm::EdDSA));
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );

        let duplicated_permission = test_claims(
            &organization_id,
            &branch_id,
            &device_id,
            vec![
                SYNC_WRITE_PERMISSION.to_owned(),
                SYNC_WRITE_PERMISSION.to_owned(),
            ],
        );
        let token = encode_ed25519(&duplicated_permission, &Header::new(Algorithm::EdDSA));
        assert_eq!(
            verifier().verify_bearer(Some(&format!("Bearer {token}"))),
            Err(DeviceAuthError::Invalid)
        );
    }
}
