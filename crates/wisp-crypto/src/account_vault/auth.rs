//! Native-only OPAQUE client/server boundary. No password or export key has a
//! wire serializer, and no failed secure login can become a legacy login here.
use crate::SecretString;
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use opaque_ke::{
    CipherSuite, ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, CredentialFinalization, CredentialRequest,
    CredentialResponse, Identifiers, RegistrationRequest, RegistrationResponse, RegistrationUpload,
    ServerLogin, ServerLoginParameters, ServerRegistration, ServerSetup,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub fn canonical_username(value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    ensure!(
        (3..=32).contains(&value.len())
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "Invalid sign-in username"
    );
    // Existing SQL uniqueness and lookup use ASCII NOCASE. Case variants must
    // share the secure-mode latch and the same OPAQUE client identifier.
    Ok(value.to_ascii_lowercase())
}

pub const SUITE: &str = "wisp-opaque-ristretto255-sha512-argon2id-v1";
pub const MAX_HANDSHAKE_WIRE: usize = 4096;

struct WispSuite;
impl CipherSuite for WispSuite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::TripleDh<opaque_ke::Ristretto255, sha2::Sha512>;
    type Ksf = Argon2<'static>;
}

fn ksf() -> Argon2<'static> {
    // Fixed by the suite identifier. Never accept server-provided cost settings.
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65_536, 3, 1, None).expect("fixed Argon2 profile"),
    )
}

fn wire(value: &str) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    ensure!(
        value.len() <= MAX_HANDSHAKE_WIRE,
        "Invalid secure authentication message"
    );
    let bytes = Zeroizing::new(
        STANDARD
            .decode(value)
            .map_err(|_| anyhow::anyhow!("Invalid secure authentication message"))?,
    );
    ensure!(
        STANDARD.encode(&*bytes) == value,
        "Noncanonical secure authentication message"
    );
    Ok(bytes)
}

pub(super) fn validate_registration_transcript(
    request: &str,
    response: &str,
    upload: &str,
) -> anyhow::Result<()> {
    RegistrationRequest::<WispSuite>::deserialize(&wire(request)?)
        .map_err(|_| anyhow::anyhow!("Invalid secure registration request"))?;
    RegistrationResponse::<WispSuite>::deserialize(&wire(response)?)
        .map_err(|_| anyhow::anyhow!("Invalid secure registration response"))?;
    RegistrationUpload::<WispSuite>::deserialize(&wire(upload)?)
        .map_err(|_| anyhow::anyhow!("Invalid secure registration upload"))?;
    Ok(())
}

fn password_allowed(password: &SecretString) -> anyhow::Result<()> {
    let value = password.expose_secret();
    ensure!(
        value.chars().count() >= 12 && value.len() <= 1024,
        "Use a password of at least 12 characters and at most 1024 bytes"
    );
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AccountContext {
    pub origin: String,
    pub network: Uuid,
    pub username: String,
}

impl AccountContext {
    pub fn validate(&self) -> anyhow::Result<()> {
        let url = url::Url::parse(&self.origin).context("Invalid account service")?;
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        ensure!(
            (url.scheme() == "https" || (url.scheme() == "http" && loopback))
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/"
                && url.origin().ascii_serialization() == self.origin,
            "Invalid account service"
        );
        ensure!(
            !self.network.is_nil() && self.username == canonical_username(&self.username)?,
            "Invalid account identity"
        );
        Ok(())
    }

    fn server_identifier(&self) -> anyhow::Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(&(SUITE, &self.origin, self.network))?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "purpose", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    /// Existing-device key unlock only; never issues a mutation grant or device.
    Unlock { device: Uuid },
    /// This credential remains inactive until this exact attempt commits.
    SignIn { device: Uuid, token_sha256: String },
    /// Digest of a canonical, versioned operation body, including its purpose,
    /// account and preconditions. Not a reusable password confirmation.
    Reauthenticate {
        device: Uuid,
        operation_sha256: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LoginContext {
    pub account: AccountContext,
    pub attempt: Uuid,
    pub credential_generation: Uuid,
    pub intent: Intent,
}

impl LoginContext {
    fn transcript(&self) -> anyhow::Result<Vec<u8>> {
        self.account.validate()?;
        let (device, digest) = match &self.intent {
            Intent::Unlock { device } => (device, None),
            Intent::SignIn {
                device,
                token_sha256,
            } => (device, Some(token_sha256)),
            Intent::Reauthenticate {
                device,
                operation_sha256,
            } => (device, Some(operation_sha256)),
        };
        ensure!(
            !self.attempt.is_nil()
                && !self.credential_generation.is_nil()
                && !device.is_nil()
                && digest.is_none_or(|digest| super::is_digest(digest)),
            "Invalid authentication purpose"
        );
        Ok(serde_json::to_vec(&("wisp-account-login-v1", self))?)
    }
}

/// Intentionally no Debug, Clone or serialization. Only the client receives it.
pub struct ExportKey(pub(super) Zeroizing<[u8; 64]>);

/// Public checkpoint retained privately with the account's secure-mode latch.
/// A fresh device learns its first checkpoint from successful authentication.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ServerPin(String);

pub struct RegistrationResult {
    pub upload: String,
    pub export_key: ExportKey,
    pub server_pin: ServerPin,
}

pub struct LoginResult {
    pub finalization: String,
    pub export_key: ExportKey,
    pub server_pin: ServerPin,
}

pub struct Registration {
    state: ClientRegistration<WispSuite>,
    password: SecretString,
}

impl Registration {
    pub fn start(password: SecretString) -> anyhow::Result<(Self, String)> {
        password_allowed(&password)?;
        let result =
            ClientRegistration::<WispSuite>::start(&mut OsRng, password.expose_secret().as_bytes())
                .map_err(|_| anyhow::anyhow!("Could not start secure registration"))?;
        let request = STANDARD.encode(result.message.serialize());
        Ok((
            Self {
                state: result.state,
                password,
            },
            request,
        ))
    }

    pub fn finish(
        self,
        context: &AccountContext,
        response: &str,
        expected_pin: Option<&ServerPin>,
    ) -> anyhow::Result<RegistrationResult> {
        let server = context.server_identifier()?;
        let response = RegistrationResponse::<WispSuite>::deserialize(&wire(response)?)
            .map_err(|_| anyhow::anyhow!("Invalid secure registration response"))?;
        let mut result = self
            .state
            .finish(
                &mut OsRng,
                self.password.expose_secret().as_bytes(),
                response,
                ClientRegistrationFinishParameters::new(
                    Identifiers {
                        client: Some(context.username.as_bytes()),
                        server: Some(&server),
                    },
                    Some(&ksf()),
                ),
            )
            .map_err(|_| anyhow::anyhow!("Could not finish secure registration"))?;
        let server_pin = ServerPin(STANDARD.encode(result.server_s_pk.serialize()));
        let key = ExportKey(Zeroizing::new(
            result
                .export_key
                .as_slice()
                .try_into()
                .expect("SHA-512 export key"),
        ));
        result.export_key.zeroize();
        ensure!(
            expected_pin.is_none_or(|pin| pin == &server_pin),
            "Secure authentication server identity changed"
        );
        Ok(RegistrationResult {
            upload: STANDARD.encode(result.message.serialize()),
            export_key: key,
            server_pin,
        })
    }
}

pub struct Login {
    state: ClientLogin<WispSuite>,
    password: SecretString,
}

impl Login {
    pub fn start(password: SecretString) -> anyhow::Result<(Self, String)> {
        password_allowed(&password)?;
        let result =
            ClientLogin::<WispSuite>::start(&mut OsRng, password.expose_secret().as_bytes())
                .map_err(|_| anyhow::anyhow!("Could not start secure sign-in"))?;
        let request = STANDARD.encode(result.message.serialize());
        Ok((
            Self {
                state: result.state,
                password,
            },
            request,
        ))
    }

    /// Authentication failure returns no export key and no legacy fallback.
    pub fn finish(
        self,
        context: &LoginContext,
        response: &str,
        expected_pin: Option<&ServerPin>,
    ) -> anyhow::Result<LoginResult> {
        let server = context.account.server_identifier()?;
        let transcript = context.transcript()?;
        let response = CredentialResponse::<WispSuite>::deserialize(&wire(response)?)
            .map_err(|_| anyhow::anyhow!("Invalid secure sign-in response"))?;
        let mut result = self
            .state
            .finish(
                &mut OsRng,
                self.password.expose_secret().as_bytes(),
                response,
                ClientLoginFinishParameters::new(
                    Some(&transcript),
                    Identifiers {
                        client: Some(context.account.username.as_bytes()),
                        server: Some(&server),
                    },
                    Some(&ksf()),
                ),
            )
            .map_err(|_| {
                anyhow::anyhow!("Secure sign-in failed. Check your username and password")
            })?;
        let server_pin = ServerPin(STANDARD.encode(result.server_s_pk.serialize()));
        let key = ExportKey(Zeroizing::new(
            result
                .export_key
                .as_slice()
                .try_into()
                .expect("SHA-512 export key"),
        ));
        result.export_key.zeroize();
        result.session_key.zeroize();
        ensure!(
            expected_pin.is_none_or(|pin| pin == &server_pin),
            "Secure authentication server identity changed"
        );
        Ok(LoginResult {
            finalization: STANDARD.encode(result.message.serialize()),
            export_key: key,
            server_pin,
        })
    }
}

pub struct Server {
    setup: ServerSetup<WispSuite>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}
impl Server {
    #[must_use]
    pub fn new() -> Self {
        Self {
            setup: ServerSetup::new(&mut OsRng),
        }
    }

    pub fn restore(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            setup: ServerSetup::deserialize(bytes)
                .map_err(|_| anyhow::anyhow!("Invalid saved secure authentication setup"))?,
        })
    }

    /// Server authentication material. Store privately; never return via an API.
    #[must_use]
    pub fn save(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.setup.serialize().to_vec())
    }

    pub fn registration_response(
        &self,
        credential_identifier: &[u8],
        request: &str,
    ) -> anyhow::Result<String> {
        ensure!(
            !credential_identifier.is_empty() && credential_identifier.len() <= 128,
            "Invalid credential binding"
        );
        let request = RegistrationRequest::<WispSuite>::deserialize(&wire(request)?)
            .map_err(|_| anyhow::anyhow!("Invalid secure registration request"))?;
        let response =
            ServerRegistration::<WispSuite>::start(&self.setup, request, credential_identifier)
                .map_err(|_| anyhow::anyhow!("Could not begin secure registration"))?;
        Ok(STANDARD.encode(response.message.serialize()))
    }

    pub fn registration_record(upload: &str) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        let upload = RegistrationUpload::<WispSuite>::deserialize(&wire(upload)?)
            .map_err(|_| anyhow::anyhow!("Invalid secure registration upload"))?;
        Ok(Zeroizing::new(
            ServerRegistration::finish(upload).serialize().to_vec(),
        ))
    }

    /// `None` uses the library's dummy-record path for an unknown account.
    pub fn login(
        &self,
        context: LoginContext,
        credential_identifier: &[u8],
        record: Option<&[u8]>,
        request: &str,
    ) -> anyhow::Result<(ServerAttempt, String)> {
        ensure!(
            !credential_identifier.is_empty() && credential_identifier.len() <= 128,
            "Invalid credential binding"
        );
        let server = context.account.server_identifier()?;
        let transcript = context.transcript()?;
        let record = record
            .map(ServerRegistration::<WispSuite>::deserialize)
            .transpose()
            .map_err(|_| anyhow::anyhow!("Invalid saved secure credential"))?;
        let request = CredentialRequest::<WispSuite>::deserialize(&wire(request)?)
            .map_err(|_| anyhow::anyhow!("Invalid secure sign-in request"))?;
        let result = ServerLogin::start(
            &mut OsRng,
            &self.setup,
            record,
            request,
            credential_identifier,
            ServerLoginParameters {
                context: Some(&transcript),
                identifiers: Identifiers {
                    client: Some(context.account.username.as_bytes()),
                    server: Some(&server),
                },
            },
        )
        .map_err(|_| anyhow::anyhow!("Could not begin secure sign-in"))?;
        let response = STANDARD.encode(result.message.serialize());
        Ok((
            ServerAttempt {
                state: result.state,
                context,
            },
            response,
        ))
    }
}

pub struct ServerAttempt {
    state: ServerLogin<WispSuite>,
    context: LoginContext,
}
impl ServerAttempt {
    /// Private pending-attempt storage only. Persistence does not replace the
    /// application's expiry, exact-operation and atomic single-use checks.
    pub fn save(&self) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        self.context.transcript()?;
        let context = serde_json::to_vec(&self.context)?;
        ensure!(
            context.len() <= 4096,
            "Invalid saved authentication context"
        );
        let state = Zeroizing::new(self.state.serialize().to_vec());
        let mut saved = Zeroizing::new(Vec::with_capacity(9 + context.len() + state.len()));
        saved.extend_from_slice(b"WSPA\x01");
        saved.extend_from_slice(&u32::try_from(context.len())?.to_be_bytes());
        saved.extend_from_slice(&context);
        saved.extend_from_slice(&state);
        Ok(saved)
    }

    pub fn restore(context: &LoginContext, bytes: &[u8]) -> anyhow::Result<Self> {
        context.transcript()?;
        ensure!(
            bytes.len() >= 9 && bytes.len() <= 8192 && &bytes[..5] == b"WSPA\x01",
            "Invalid saved authentication attempt"
        );
        let length = usize::try_from(u32::from_be_bytes(
            bytes[5..9].try_into().expect("checked length"),
        ))?;
        ensure!(
            length <= 4096 && length < bytes.len() - 9,
            "Invalid saved authentication context"
        );
        let saved_context: &[u8] = &bytes[9..9 + length];
        super::strict_json::validate(saved_context)?;
        let saved_context: LoginContext = serde_json::from_slice(saved_context)
            .map_err(|_| anyhow::anyhow!("Invalid saved authentication context"))?;
        ensure!(
            context == &saved_context,
            "Saved authentication attempt belongs to a different context"
        );
        Ok(Self {
            state: ServerLogin::deserialize(&bytes[9 + length..])
                .map_err(|_| anyhow::anyhow!("Invalid saved secure authentication attempt"))?,
            context: saved_context,
        })
    }

    pub fn finish(self, finalization: &str) -> anyhow::Result<()> {
        let server = self.context.account.server_identifier()?;
        let transcript = self.context.transcript()?;
        let finalization = CredentialFinalization::<WispSuite>::deserialize(&wire(finalization)?)
            .map_err(|_| anyhow::anyhow!("Invalid secure sign-in proof"))?;
        let mut result = self
            .state
            .finish(
                finalization,
                ServerLoginParameters {
                    context: Some(&transcript),
                    identifiers: Identifiers {
                        client: Some(self.context.account.username.as_bytes()),
                        server: Some(&server),
                    },
                },
            )
            .map_err(|_| anyhow::anyhow!("Secure sign-in failed"))?;
        result.session_key.zeroize();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::manual_assert_eq)] // Do not print even synthetic private keys on failure.
mod tests {
    use super::*;

    const PASSWORD: &str = "synthetic fixture password only";

    fn context() -> LoginContext {
        LoginContext {
            account: AccountContext {
                origin: "https://example.invalid".into(),
                network: Uuid::new_v4(),
                username: "canonicalname".into(),
            },
            attempt: Uuid::new_v4(),
            credential_generation: Uuid::new_v4(),
            intent: Intent::SignIn {
                device: Uuid::new_v4(),
                token_sha256: "a".repeat(64),
            },
        }
    }

    fn register(
        server: &Server,
        context: &AccountContext,
    ) -> (Zeroizing<Vec<u8>>, RegistrationResult) {
        let (client, request) =
            Registration::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        let response = server
            .registration_response(b"test-account-generation", &request)
            .unwrap();
        let result = client.finish(context, &response, None).unwrap();
        (Server::registration_record(&result.upload).unwrap(), result)
    }

    #[test]
    fn secure_sign_in_restores_export_key_and_consumes_server_attempt() {
        let server = Server::new();
        let context = context();
        let (record, registration) = register(&server, &context.account);
        let server = Server::restore(&server.save()).unwrap();
        let (client, request) = Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        let (attempt, response) = server
            .login(
                context.clone(),
                b"test-account-generation",
                Some(&record),
                &request,
            )
            .unwrap();
        let saved = attempt.save().unwrap();
        for field in 0..4 {
            let mut changed = context.clone();
            match field {
                0 => changed.attempt = Uuid::new_v4(),
                1 => changed.credential_generation = Uuid::new_v4(),
                2 => {
                    changed.intent = Intent::Reauthenticate {
                        device: Uuid::new_v4(),
                        operation_sha256: "b".repeat(64),
                    }
                }
                _ => {
                    changed.intent = Intent::SignIn {
                        device: Uuid::new_v4(),
                        token_sha256: "a".repeat(64),
                    }
                }
            }
            assert!(ServerAttempt::restore(&changed, &saved).is_err());
        }
        let attempt = ServerAttempt::restore(&context, &saved).unwrap();
        let result = client
            .finish(&context, &response, Some(&registration.server_pin))
            .unwrap();
        assert!(*registration.export_key.0 == *result.export_key.0);
        assert_eq!(registration.server_pin, result.server_pin);
        attempt.finish(&result.finalization).unwrap();
    }

    #[test]
    fn wrong_password_and_unknown_account_fail_with_no_export_key() {
        let server = Server::new();
        let context = context();
        let (record, _) = register(&server, &context.account);
        let mut lengths = Vec::new();
        for record in [Some(record.as_slice()), None] {
            let (client, request) =
                Login::start(SecretString::from("wrong synthetic password".to_owned())).unwrap();
            let (_, response) = server
                .login(
                    context.clone(),
                    b"test-account-generation",
                    record,
                    &request,
                )
                .unwrap();
            lengths.push(response.len());
            assert!(client.finish(&context, &response, None).is_err());
        }
        assert_eq!(lengths[0], lengths[1]);
    }

    #[test]
    fn login_binds_origin_network_username_attempt_generation_and_device_intent() {
        let server = Server::new();
        let context = context();
        let (record, _) = register(&server, &context.account);
        for mutation in 0..8 {
            let (client, request) = Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
            let (_, response) = server
                .login(
                    context.clone(),
                    b"test-account-generation",
                    Some(&record),
                    &request,
                )
                .unwrap();
            let mut changed = context.clone();
            match mutation {
                0 => changed.account.origin = "https://other.invalid".into(),
                1 => changed.account.network = Uuid::new_v4(),
                2 => changed.account.username = "othername".into(),
                3 => changed.attempt = Uuid::new_v4(),
                4 => changed.credential_generation = Uuid::new_v4(),
                5 => {
                    changed.intent = Intent::SignIn {
                        device: Uuid::new_v4(),
                        token_sha256: "a".repeat(64),
                    }
                }
                6 => {
                    changed.intent = Intent::SignIn {
                        device: match context.intent {
                            Intent::SignIn { device, .. } => device,
                            Intent::Reauthenticate { .. } | Intent::Unlock { .. } => unreachable!(),
                        },
                        token_sha256: "b".repeat(64),
                    }
                }
                _ => {
                    changed.intent = Intent::Reauthenticate {
                        device: Uuid::new_v4(),
                        operation_sha256: "a".repeat(64),
                    }
                }
            }
            assert!(client.finish(&changed, &response, None).is_err());
        }
    }

    #[test]
    fn server_pin_change_and_proof_from_another_attempt_fail() {
        let server = Server::new();
        let context = context();
        let (record, registration) = register(&server, &context.account);
        let (client, request) = Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        let (_, response) = server
            .login(
                context.clone(),
                b"test-account-generation",
                Some(&record),
                &request,
            )
            .unwrap();
        assert!(
            client
                .finish(&context, &response, Some(&ServerPin("changed".into())))
                .is_err()
        );
        let (client, request) = Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        let (_, response) = server
            .login(
                context.clone(),
                b"test-account-generation",
                Some(&record),
                &request,
            )
            .unwrap();
        let result = client
            .finish(&context, &response, Some(&registration.server_pin))
            .unwrap();
        let (_, request) = Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        let (other, _) = server
            .login(context, b"test-account-generation", Some(&record), &request)
            .unwrap();
        assert!(other.finish(&result.finalization).is_err());
    }

    #[test]
    fn malformed_wire_and_noncanonical_origins_are_rejected() {
        assert_eq!(canonical_username("  MixedCase  ").unwrap(), "mixedcase");
        let mut upper = context();
        upper.account.username = "MixedCase".into();
        assert!(upper.transcript().is_err());
        assert!(wire(&"a".repeat(MAX_HANDSHAKE_WIRE + 1)).is_err());
        assert!(wire("AA").is_err());
        for origin in [
            "https://example.invalid/",
            "https://example.invalid/path",
            "https://user@example.invalid",
            "http://example.invalid",
            "https://example.invalid:443",
        ] {
            let mut context = context();
            context.account.origin = origin.into();
            assert!(context.transcript().is_err());
        }
    }
}
