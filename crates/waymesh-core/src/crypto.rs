use crate::error::{CoreError, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::Rng;
use sha2::{Digest, Sha256};

pub struct KeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.as_bytes())
    }

    pub fn sign_message(&self, message: &[u8]) -> String {
        let signature = self.signing_key.sign(message);
        hex::encode(signature.to_bytes())
    }
}

pub fn verify_signature(public_key_hex: &str, message: &[u8], signature_hex: &str) -> Result<()> {
    let pk_bytes = hex::decode(public_key_hex).map_err(|e| CoreError::KeyError(e.to_string()))?;
    let sig_bytes = hex::decode(signature_hex).map_err(|e| CoreError::KeyError(e.to_string()))?;

    let pk_array: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| CoreError::KeyError("Public key must be 32 bytes".into()))?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| CoreError::KeyError("Signature must be 64 bytes".into()))?;

    let verifying_key =
        VerifyingKey::from_bytes(&pk_array).map_err(|e| CoreError::KeyError(e.to_string()))?;
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(message, &signature)
        .map_err(|_| CoreError::InvalidSignature)
}

/// Generates a secure, human-friendly 6-digit numeric OTP token
pub fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    let code: u32 = rng.gen_range(100_000..=999_999);
    code.to_string()
}

/// Computes SHA-256 hash of an OTP token
pub fn hash_otp(otp: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(otp.trim().as_bytes());
    hex::encode(hasher.finalize())
}

/// Constant-time byte equality check to prevent timing side-channel attacks
fn constant_time_eq_slices(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x.to_ascii_lowercase() ^ y.to_ascii_lowercase();
    }
    diff == 0
}

/// Verifies whether a plaintext OTP matches a stored hash in constant time
pub fn verify_otp(otp_input: &str, expected_hash: &str) -> Result<()> {
    let input_hash = hash_otp(otp_input);
    if constant_time_eq_slices(input_hash.as_bytes(), expected_hash.as_bytes()) {
        Ok(())
    } else {
        Err(CoreError::OtpMismatch)
    }
}

mod hex {
    pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
        let bytes = data.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    pub fn decode(hex_str: &str) -> Result<Vec<u8>, &'static str> {
        if !hex_str.len().is_multiple_of(2) {
            return Err("Hex string has odd length");
        }
        let mut bytes = Vec::with_capacity(hex_str.len() / 2);
        for i in (0..hex_str.len()).step_by(2) {
            let byte =
                u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|_| "Invalid hex character")?;
            bytes.push(byte);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otp_generation_and_verification() {
        let otp = generate_otp();
        assert_eq!(otp.len(), 6);
        let hash = hash_otp(&otp);

        assert!(verify_otp(&otp, &hash).is_ok());
        assert!(verify_otp("000000", &hash).is_err());
    }

    #[test]
    fn test_signature_roundtrip() {
        let keypair = KeyPair::generate();
        let pk_hex = keypair.public_key_hex();
        let message = b"waymesh-handover-test";
        let sig_hex = keypair.sign_message(message);

        assert!(verify_signature(&pk_hex, message, &sig_hex).is_ok());
        assert!(verify_signature(&pk_hex, b"tampered-message", &sig_hex).is_err());
    }
}
