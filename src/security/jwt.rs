use chrono::{Duration, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Access tokens are short-lived (30 min). Refresh tokens are long-lived (7 days)
/// and tracked in `admin_refresh_tokens` so they can be revoked (logout, kick).
pub const ACCESS_TOKEN_TTL_MIN: i64 = 30;
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 7;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: i64, // admin_id
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub typ: String, // "access"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: i64,
    pub jti: String,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub typ: String, // "refresh"
}

pub struct Jwt {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Jwt {
    pub fn from_app_secret(secret: &str) -> Self {
        // 256-bit HS256 key derived from app_secret (same root of trust as SecretBox)
        let mut h = Sha256::new();
        h.update(b"dujiao-rust/jwt/v1/");
        h.update(secret.as_bytes());
        let key = h.finalize();
        Self {
            encoding: EncodingKey::from_secret(&key),
            decoding: DecodingKey::from_secret(&key),
        }
    }

    pub fn sign_access(&self, admin_id: i64, role: &str) -> anyhow::Result<String> {
        let now = Utc::now();
        let claims = AccessClaims {
            sub: admin_id,
            role: role.to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(ACCESS_TOKEN_TTL_MIN)).timestamp(),
            typ: "access".to_string(),
        };
        Ok(encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &self.encoding,
        )?)
    }

    pub fn sign_refresh(&self, admin_id: i64, jti: &str) -> anyhow::Result<(String, i64)> {
        let now = Utc::now();
        let exp = (now + Duration::days(REFRESH_TOKEN_TTL_DAYS)).timestamp();
        let claims = RefreshClaims {
            sub: admin_id,
            jti: jti.to_string(),
            iat: now.timestamp(),
            exp,
            typ: "refresh".to_string(),
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)?;
        Ok((token, exp))
    }

    pub fn verify_access(
        &self,
        token: &str,
    ) -> jsonwebtoken::errors::Result<TokenData<AccessClaims>> {
        let validation = Validation::new(Algorithm::HS256);
        decode::<AccessClaims>(token, &self.decoding, &validation)
    }

    pub fn verify_refresh(
        &self,
        token: &str,
    ) -> jsonwebtoken::errors::Result<TokenData<RefreshClaims>> {
        let validation = Validation::new(Algorithm::HS256);
        decode::<RefreshClaims>(token, &self.decoding, &validation)
    }
}

pub fn new_jti() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_roundtrip() {
        let jwt = Jwt::from_app_secret("k");
        let token = jwt.sign_access(7, "owner").unwrap();
        let data = jwt.verify_access(&token).unwrap();
        assert_eq!(data.claims.sub, 7);
        assert_eq!(data.claims.role, "owner");
        assert_eq!(data.claims.typ, "access");
    }

    #[test]
    fn refresh_roundtrip() {
        let jwt = Jwt::from_app_secret("k");
        let (token, exp) = jwt.sign_refresh(7, "abc").unwrap();
        let data = jwt.verify_refresh(&token).unwrap();
        assert_eq!(data.claims.sub, 7);
        assert_eq!(data.claims.jti, "abc");
        assert_eq!(data.claims.exp, exp);
    }

    #[test]
    fn different_secret_fails() {
        let a = Jwt::from_app_secret("a");
        let b = Jwt::from_app_secret("b");
        let token = a.sign_access(1, "owner").unwrap();
        assert!(b.verify_access(&token).is_err());
    }
}
